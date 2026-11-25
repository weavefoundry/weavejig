//! Formatting rules: syntax tree in, layout document out.
//!
//! [`Formatter::node`] dispatches on the node kind. Kinds without a rule of
//! their own fall back to [`Formatter::generic`], which lays the node's
//! children out on one line using the spacing table. A rule exists wherever
//! a construct needs line breaks or indentation.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use weavejig_syntax::{
    NodeOrToken, SyntaxElement, SyntaxKind, SyntaxKind::*, SyntaxNode, SyntaxToken, TextSize,
    first_token, last_token, significant_children,
};

use crate::FormatOptions;
use crate::comments::{Comment, Comments, TokenTrivia};
use crate::doc::{
    Doc, Tag, concat, fill_break, group, group_if, indent, indent_if, next_line_if_fits, space,
    tagged_group, text,
};
use crate::imports::import_name;
use crate::spacing::space_between;

pub(crate) struct Formatter<'a> {
    comments: Comments,
    options: &'a FormatOptions,
    /// A token whose leading comments have already been printed elsewhere.
    skip_leading: Cell<Option<TextSize>>,
    /// Simple names of imported annotations that are known to annotate types
    /// rather than declarations, such as JSpecify's `@Nullable`.
    type_annotations: HashSet<String>,
    next_tag: Cell<Tag>,
    /// Argument lists, by start offset, whose only argument is not laid out
    /// the special way a lone lambda usually is.
    plain_lists: RefCell<HashSet<TextSize>>,
    /// The offset at which each line of the source starts.
    line_starts: Vec<TextSize>,
    source: String,
}

/// Annotations that google-java-format keeps next to the type they annotate
/// when they are imported by name.
const TYPE_ANNOTATIONS: [&str; 4] = [
    "org.jspecify.annotations.Nullable",
    "org.jspecify.annotations.NonNull",
    "org.checkerframework.checker.nullness.qual.Nullable",
    "org.checkerframework.checker.nullness.qual.NonNull",
];

/// Whether an annotation is given any values: `@A(1)`, but not `@A()`.
fn has_arguments(annotation: &SyntaxElement) -> bool {
    annotation.as_node().is_some_and(|a| {
        a.children()
            .filter(|c| c.kind() == ANNOTATION_ARGS)
            .any(|args| significant_children(&args).count() > 2)
    })
}

fn first_of(element: &SyntaxElement) -> Option<SyntaxToken> {
    match element {
        NodeOrToken::Node(node) => first_token(node),
        NodeOrToken::Token(token) => Some(token.clone()),
    }
}

fn last_of(element: &SyntaxElement) -> Option<SyntaxToken> {
    match element {
        NodeOrToken::Node(node) => last_token(node),
        NodeOrToken::Token(token) => Some(token.clone()),
    }
}

fn comment_doc(comment: &Comment, trailing: bool) -> Doc {
    if comment.is_line {
        let mut line = comment.text.trim_end().to_string();
        // `//comment` gets a space after the slashes, except in the two
        // forms tools read: `//noinspection` and `//$NON-NLS-1$`.
        let slashes = line.len() - line.trim_start_matches('/').len();
        let body = &line[slashes..];
        if !body.is_empty()
            && !body.starts_with(char::is_whitespace)
            && !body.starts_with("noinspection")
            && !is_non_nls_marker(body)
        {
            line.insert(slashes, ' ');
        }
        return if trailing {
            Doc::Suffix(line)
        } else if comment.newlines_before > 0 {
            Doc::LineComment(line)
        } else {
            text(line)
        };
    }
    if comment.is_javadoc() && !comment.text.starts_with("/***") {
        return Doc::Javadoc(comment.text.clone());
    }
    let lines: Vec<&str> = comment.text.lines().collect();
    if lines.len() < 2 {
        return text(comment.text.clone());
    }
    let rest = &lines[1..];
    let mut docs = vec![text(lines[0].trim_end())];
    // As google-java-format does: if every later line starts with `*`, they
    // line up one column in from the comment's start. Otherwise the comment
    // keeps its shape: later lines lose only the indentation they all share,
    // so commented-out code and diagrams stay intact.
    if rest.iter().all(|line| line.trim_start().starts_with('*')) {
        for line in rest {
            docs.push(Doc::HardLine);
            docs.push(text(format!(" {}", line.trim())));
        }
    } else {
        let shared = rest
            .iter()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.len() - line.trim_start().len())
            .min()
            .unwrap_or(0);
        for line in rest {
            docs.push(Doc::HardLine);
            let line = line.trim_end();
            docs.push(text(line.get(shared..).unwrap_or(line.trim_start())));
        }
    }
    concat(docs)
}

impl<'a> Formatter<'a> {
    pub(crate) fn new(root: &SyntaxNode, options: &'a FormatOptions) -> Formatter<'a> {
        let type_annotations = root
            .children()
            .filter(|child| child.kind() == IMPORT_DECL)
            .map(|import| import_name(&import))
            .filter(|(is_static, name)| !is_static && TYPE_ANNOTATIONS.contains(&name.as_str()))
            .filter_map(|(_, name)| name.rsplit('.').next().map(str::to_string))
            .collect();
        let source = root.text().to_string();
        let line_starts = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(newline, _)| newline + 1))
            .filter_map(|start| TextSize::try_from(start).ok())
            .collect();
        Formatter {
            line_starts,
            source,
            comments: Comments::collect(root),
            options,
            skip_leading: Cell::new(None),
            type_annotations,
            next_tag: Cell::new(0),
            plain_lists: RefCell::new(HashSet::new()),
        }
    }

    fn tag(&self) -> Tag {
        let tag = self.next_tag.get();
        self.next_tag.set(tag + 1);
        tag
    }

    /// The column of a token in the source, counted in characters from 0.
    fn source_column(&self, token: &SyntaxToken) -> usize {
        let offset = token.text_range().start();
        let line = self.line_starts.partition_point(|start| *start <= offset);
        let start = self.line_starts[line.saturating_sub(1)];
        self.source[usize::from(start)..usize::from(offset)]
            .chars()
            .count()
    }

    fn block_indent(&self) -> u16 {
        self.options.style.block_indent()
    }

    fn continuation_indent(&self) -> u16 {
        self.options.style.continuation_indent()
    }

    // --- Tokens and comments ---------------------------------------------

    fn trivia(&self, token: &SyntaxToken) -> Option<&TokenTrivia> {
        self.comments.of(token.text_range().start())
    }

    /// Whether the source has an empty line before `token` (or before the
    /// comments attached in front of it).
    fn blank_line_before(&self, token: &SyntaxToken) -> bool {
        self.trivia(token)
            .is_some_and(TokenTrivia::blank_line_before)
    }

    /// A line break, or a blank line if the source has one before `node`.
    fn separator(&self, node: &SyntaxNode) -> Doc {
        match first_token(node) {
            Some(first) if self.blank_line_before(&first) => Doc::BlankLine,
            _ => Doc::HardLine,
        }
    }

    fn token(&self, token: &SyntaxToken) -> Doc {
        let token_text = |token: &SyntaxToken| match token.kind() {
            TEXT_BLOCK => Doc::TextBlock(token.text().to_string()),
            _ => text(token.text()),
        };
        let Some(trivia) = self.trivia(token) else {
            return token_text(token);
        };
        let mut docs = Vec::new();
        if self.skip_leading.get() != Some(token.text_range().start()) {
            // `null /* why */)`: a comment after code on its line is set
            // off from it, except right after an opening bracket.
            let same_line = trivia
                .leading
                .first()
                .is_some_and(|comment| comment.newlines_before == 0);
            let after_open = weavejig_syntax::prev_significant_token(token)
                .is_none_or(|previous| matches!(previous.kind(), L_PAREN | L_BRACK));
            if same_line && !after_open {
                docs.push(space());
            }
            self.leading_comments_spaced(trivia, Some(token.kind()), true, &mut docs);
        }
        docs.push(token_text(token));
        for comment in &trivia.trailing {
            docs.push(space());
            docs.push(comment_doc(comment, true));
            if comment.is_line {
                docs.push(Doc::CommentBreak);
            }
        }
        concat(docs)
    }

    /// The token without its leading comments, for a closing brace whose
    /// comments were printed inside the block.
    fn token_without_leading(&self, token: &SyntaxToken) -> Doc {
        let previous = self.skip_leading.replace(Some(token.text_range().start()));
        let doc = self.token(token);
        self.skip_leading.set(previous);
        doc
    }

    /// Prints the comments in front of a token of kind `before` (`None` for
    /// a closing brace, whose comments end the block's contents).
    fn leading_comments(
        &self,
        trivia: &TokenTrivia,
        before: Option<SyntaxKind>,
        docs: &mut Vec<Doc>,
    ) {
        self.leading_comments_spaced(trivia, before, false, docs);
    }

    /// [`Formatter::leading_comments`], which leaves a blank line before the
    /// first comment to the caller. With `keep_blank`, it is kept here: the
    /// comment is inside an expression, where nothing else would.
    fn leading_comments_spaced(
        &self,
        trivia: &TokenTrivia,
        before: Option<SyntaxKind>,
        keep_blank: bool,
        docs: &mut Vec<Doc>,
    ) {
        for (i, comment) in trivia.leading.iter().enumerate() {
            if comment.newlines_before > 0 {
                let blank = (i > 0 || keep_blank) && comment.newlines_before >= 2;
                docs.push(if blank { Doc::BlankLine } else { Doc::HardLine });
            }
            docs.push(comment_doc(comment, false));
            let next = trivia.leading.get(i + 1);
            let newlines_after = next.map_or(trivia.newlines_before, |c| c.newlines_before);
            if next.is_some() {
                if newlines_after == 0 {
                    docs.push(space());
                }
            } else if newlines_after >= 2 && !(comment.is_javadoc() && before.is_some()) {
                docs.push(Doc::BlankLine);
            } else if newlines_after >= 1 || comment.is_line {
                docs.push(Doc::HardLine);
            } else if !matches!(before, Some(COMMA | SEMI | R_PAREN | R_BRACK)) {
                docs.push(space());
            }
        }
    }

    /// Comments before a closing brace, printed at the indentation of the
    /// block's contents.
    fn dangling_comments(&self, close: &SyntaxToken, docs: &mut Vec<Doc>) {
        let Some(trivia) = self.trivia(close).filter(|t| !t.leading.is_empty()) else {
            return;
        };
        let blank = trivia.blank_line_before() && !docs.is_empty();
        docs.push(if blank { Doc::BlankLine } else { Doc::HardLine });
        self.leading_comments(trivia, None, docs);
    }

    fn has_leading_comments(&self, node: &SyntaxNode) -> bool {
        first_token(node)
            .and_then(|first| self.trivia(&first).map(|t| !t.leading.is_empty()))
            .unwrap_or(false)
    }

    fn has_dangling_comments(&self, close: &SyntaxToken) -> bool {
        self.trivia(close).is_some_and(|t| !t.leading.is_empty())
    }

    /// A statement or declaration with the comments in front of it printed
    /// first, outside the element's own layout. A comment on its own line
    /// must not make the code after it wrap.
    fn with_comments(&self, element: &SyntaxElement) -> Doc {
        let (mut docs, doc) = self.split_comments(element);
        docs.push(doc);
        concat(docs)
    }

    /// The comments in front of `element`, and `element` without them, for
    /// a caller that puts the element in a group the comments must stay
    /// out of.
    fn split_comments(&self, element: &SyntaxElement) -> (Vec<Doc>, Doc) {
        let Some(first) = first_of(element) else {
            return (Vec::new(), self.element(element));
        };
        let Some(trivia) = self.trivia(&first).filter(|t| !t.leading.is_empty()) else {
            return (Vec::new(), self.element(element));
        };
        // An enclosing element that starts with the same token has printed
        // them already.
        if self.skip_leading.get() == Some(first.text_range().start()) {
            return (Vec::new(), self.element(element));
        }
        let mut comments = Vec::new();
        self.leading_comments(trivia, Some(first.kind()), &mut comments);
        let previous = self.skip_leading.replace(Some(first.text_range().start()));
        let doc = self.element(element);
        self.skip_leading.set(previous);
        (comments, doc)
    }

    // --- Generic layout ----------------------------------------------------

    fn element(&self, element: &SyntaxElement) -> Doc {
        match element {
            NodeOrToken::Node(node) => self.node(node),
            NodeOrToken::Token(token) => self.token(token),
        }
    }

    /// Sibling elements on one line, separated according to the spacing table.
    fn spaced(&self, elements: &[SyntaxElement]) -> Doc {
        let mut docs = Vec::with_capacity(elements.len() * 2);
        let mut previous: Option<SyntaxToken> = None;
        for element in elements {
            let Some(first) = first_of(element) else {
                continue;
            };
            if let Some(previous) = &previous {
                // `f(a /* why */, b)`: a comment in front of punctuation still
                // gets a space before it.
                let comment_first = !matches!(previous.kind(), L_PAREN | L_BRACK)
                    && self.trivia(&first).is_some_and(|t| !t.leading.is_empty());
                if comment_first || space_between(previous, &first) {
                    docs.push(space());
                }
            }
            docs.push(self.element(element));
            previous = last_of(element);
        }
        concat(docs)
    }

    fn generic(&self, node: &SyntaxNode) -> Doc {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        self.spaced(&elements)
    }

    pub(crate) fn node(&self, node: &SyntaxNode) -> Doc {
        // Comments in front of an expression come before it, outside its
        // layout: a comment on a line of its own must not make the
        // expression wrap, nor take on the indentation of its later lines.
        if is_expression(node.kind())
            && let Some(first) = first_token(node)
            && self.skip_leading.get() != Some(first.text_range().start())
            && let Some(trivia) = self.trivia(&first)
            && trivia
                .leading
                .iter()
                .any(|comment| comment.newlines_before > 0)
        {
            let mut docs = Vec::new();
            self.leading_comments_spaced(trivia, Some(first.kind()), true, &mut docs);
            let previous = self.skip_leading.replace(Some(first.text_range().start()));
            docs.push(self.laid_out(node));
            self.skip_leading.set(previous);
            return concat(docs);
        }
        self.laid_out(node)
    }

    fn laid_out(&self, node: &SyntaxNode) -> Doc {
        let doc = match node.kind() {
            COMPILATION_UNIT => Some(self.compilation_unit(node)),
            MODULE_DECL => self.module_decl(node),
            MODULE_DIRECTIVE => self.module_directive(node),
            MODIFIERS => Some(self.modifiers(node)),
            CLASS_DECL | INTERFACE_DECL | ENUM_DECL | RECORD_DECL | ANNOTATION_TYPE_DECL => {
                self.type_decl(node)
            }
            METHOD_DECL | CONSTRUCTOR_DECL => self.method_decl(node),
            FIELD_DECL | LOCAL_VAR_DECL => self.var_decl(node),
            PARAM => self.param(node),
            TYPE_ARGS | TYPE_PARAMS => self.angle_list(node),
            UNION_TYPE => self.union_type(node),
            CLASS_BODY => self.class_body(node),
            BLOCK | SWITCH_BLOCK => self.block(node),
            SWITCH_GROUP => Some(self.switch_group(node)),
            SWITCH_RULE => self.switch_rule(node),
            CASE_LABEL => self.case_label(node, self.tag()),
            IF_STMT => self.if_stmt(node),
            WHILE_STMT => self.loop_stmt(node),
            DO_STMT => self.do_stmt(node),
            FOR_STMT => self.for_stmt(node),
            FOREACH_STMT => self.foreach_stmt(node),
            LABELED_STMT => self.labeled_stmt(node),
            RESOURCE_LIST => self.resource_list(node),
            RESOURCE => self.resource(node),
            VARIABLE_DECLARATOR | ASSIGN_EXPR | ANNOTATION_PAIR => self.assignment(node),
            BINARY_EXPR => self.binary(node),
            CONDITIONAL_EXPR => self.conditional(node),
            METHOD_CALL | FIELD_ACCESS => Some(self.chain(node)),
            ARG_LIST | PARAM_LIST | LAMBDA_PARAMS | ANNOTATION_ARGS => self.list(node, None),
            CAST_EXPR => self.cast(node),
            INSTANCEOF_EXPR => self.instance_of(node),
            METHOD_REF => self.method_ref(node),
            CLASS_LITERAL => self.class_literal(node),
            TYPE_PARAM => self.type_param(node),
            ARRAY_INIT => self.array_init(node),
            LAMBDA_EXPR => self.lambda(node),
            _ => None,
        };
        // A rule returns `None` for a shape it does not expect, which the
        // generic layout then handles.
        doc.unwrap_or_else(|| self.generic(node))
    }

    // --- Compilation units -------------------------------------------------

    fn compilation_unit(&self, root: &SyntaxNode) -> Doc {
        let children: Vec<SyntaxNode> = root.children().collect();
        let imports: Vec<&SyntaxNode> = children
            .iter()
            .filter(|child| child.kind() == IMPORT_DECL)
            .collect();
        let mut docs = Vec::new();
        let mut imports_done = false;
        for child in &children {
            if child.kind() == IMPORT_DECL {
                if !imports_done {
                    docs.push(Doc::BlankLine);
                    docs.push(self.imports(&imports));
                    imports_done = true;
                }
                continue;
            }
            docs.push(Doc::BlankLine);
            docs.push(self.with_comments(&NodeOrToken::Node(child.clone())));
        }
        for comment in &self.comments.at_end {
            docs.push(if comment.newlines_before >= 2 {
                Doc::BlankLine
            } else {
                Doc::HardLine
            });
            docs.push(comment_doc(comment, false));
        }
        concat(docs)
    }

    /// Imports in google-java-format order: static imports, a blank line,
    /// then the rest; each block sorted by name, duplicates removed.
    fn imports(&self, imports: &[&SyntaxNode]) -> Doc {
        let mut docs = Vec::new();
        // Comments above the first import usually describe the file or the
        // whole import section, so they stay on top whatever sorts first.
        let first = imports.first().and_then(|import| first_token(import));
        if let Some(trivia) = first.as_ref().and_then(|first| self.trivia(first)) {
            self.leading_comments(trivia, Some(IMPORT_KW), &mut docs);
        }
        self.skip_leading
            .set(first.map(|first| first.text_range().start()));

        let mut entries: Vec<(bool, String, &SyntaxNode)> = imports
            .iter()
            .map(|import| {
                let (is_static, name) = import_name(import);
                (!is_static, name, *import)
            })
            .collect();
        entries.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
        entries.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);

        let mut previous_static = None;
        for (non_static, _, import) in entries {
            let changed_block = previous_static.is_some_and(|previous| previous != non_static);
            previous_static = Some(non_static);
            if !changed_block {
                docs.push(Doc::HardLine);
                docs.push(self.generic(import));
                continue;
            }
            // Comments between imports belong to the import above them (see
            // `imports`), so those before the first non-static import stay
            // with the static ones, above the blank line.
            let start = first_token(import).map(|token| token.text_range().start());
            if let Some(start) = start.filter(|start| self.skip_leading.get() != Some(*start))
                && let Some(trivia) = self.comments.of(start)
                && !trivia.leading.is_empty()
            {
                docs.push(Doc::HardLine);
                self.leading_comments(trivia, Some(IMPORT_KW), &mut docs);
                docs.push(Doc::BlankLine);
                let previous = self.skip_leading.replace(Some(start));
                docs.push(self.generic(import));
                self.skip_leading.set(previous);
                continue;
            }
            docs.push(Doc::BlankLine);
            docs.push(self.generic(import));
        }
        self.skip_leading.set(None);
        concat(docs)
    }

    fn module_decl(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let open = elements.iter().position(|el| el.kind() == L_BRACE)?;
        let close = elements
            .last()?
            .as_token()
            .filter(|t| t.kind() == R_BRACE)?;
        if open + 2 == elements.len() && !self.has_dangling_comments(close) {
            return Some(concat(vec![
                self.spaced(&elements[..=open]),
                self.token(close),
            ]));
        }
        // Directives of one kind (`requires`, `exports`, ...) are packed
        // together, and a blank line sets each kind apart from the next,
        // whatever blank lines the source has.
        let mut body = Vec::new();
        let mut previous: Option<String> = None;
        for directive in elements[open + 1..elements.len() - 1].iter() {
            let kind = first_of(directive).map(|first| first.text().to_string());
            // A blank line above a comment is the author's, and stays.
            let commented = directive
                .as_node()
                .is_some_and(|node| self.has_leading_comments(node));
            body.push(match (&previous, directive.as_node()) {
                (Some(previous), _) if Some(previous) != kind.as_ref() => Doc::BlankLine,
                (Some(_), Some(node)) if commented => self.separator(node),
                _ => Doc::HardLine,
            });
            body.push(self.with_comments(directive));
            previous = kind;
        }
        self.dangling_comments(close, &mut body);
        Some(concat(vec![
            self.spaced(&elements[..=open]),
            indent(self.block_indent(), concat(body)),
            Doc::HardLine,
            self.token_without_leading(close),
        ]))
    }

    /// `exports a.b to c, d;`: the names after `to` or `with` take a line
    /// each, under the directive.
    fn module_directive(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let is_word = |el: &SyntaxElement, words: &[&str]| {
            el.as_token()
                .is_some_and(|token| token.kind() == IDENT && words.contains(&token.text()))
        };
        if !is_word(elements.first()?, &["exports", "opens", "provides"]) {
            return None;
        }
        // The first `to` or `with` that is a word of its own, not part of
        // a dotted name.
        let keyword = (2..elements.len()).find(|&i| {
            is_word(&elements[i], &["to", "with"])
                && elements[i - 1].kind() != DOT
                && elements.get(i + 1).is_some_and(|next| next.kind() != DOT)
        })?;
        let names: Vec<&[SyntaxElement]> = elements[keyword + 1..]
            .split_inclusive(|el| el.kind() == COMMA)
            .collect();
        // Each name takes a line, even when there is only one.
        let mut list = Vec::new();
        for name in &names {
            list.push(Doc::HardLine);
            list.push(self.spaced(name));
        }
        Some(group(concat(vec![
            self.spaced(&elements[..=keyword]),
            indent(self.continuation_indent(), concat(list)),
        ])))
    }

    // --- Declarations ------------------------------------------------------

    /// How many of the annotations at the front of a modifier list go on
    /// lines of their own, above the declaration.
    ///
    /// Annotations that follow a keyword (`public @Nullable String`) always
    /// stay inline, and so do known type annotations that come right before
    /// the type. Of the rest:
    ///
    /// - on a class, method, constructor or enum constant, all of them get a
    ///   line each;
    /// - on a field, all or none do: all if any annotation takes arguments;
    /// - on a local variable or resource, all or none do: none only for a
    ///   single annotation without arguments;
    /// - on a parameter or a `for` variable, none do.
    fn annotation_lines(&self, modifiers: &SyntaxNode) -> usize {
        let elements: Vec<SyntaxElement> = significant_children(modifiers).collect();
        let leading = elements
            .iter()
            .take_while(|el| el.kind() == ANNOTATION)
            .count();
        let Some(parent) = modifiers.parent() else {
            return 0;
        };
        let kind = parent.kind();
        let typed = matches!(kind, METHOD_DECL | FIELD_DECL | LOCAL_VAR_DECL | RESOURCE);
        let type_use = if typed && leading == elements.len() {
            elements
                .iter()
                .rev()
                .take_while(|el| self.is_type_annotation(el))
                .count()
        } else {
            0
        };
        let declaration = leading - type_use;
        let any_arguments = elements[..declaration].iter().any(has_arguments)
            || elements[leading..].iter().any(has_arguments);
        let vertical = match kind {
            CLASS_DECL | INTERFACE_DECL | ENUM_DECL | RECORD_DECL | ANNOTATION_TYPE_DECL
            | METHOD_DECL | CONSTRUCTOR_DECL | ENUM_CONSTANT | PACKAGE_DECL | MODULE_DECL => true,
            FIELD_DECL => any_arguments,
            LOCAL_VAR_DECL => {
                let in_foreach = parent.parent().is_some_and(|p| p.kind() == FOREACH_STMT);
                !in_foreach && (declaration > 1 || any_arguments)
            }
            RESOURCE => declaration > 1 || any_arguments,
            _ => false,
        };
        if vertical { declaration } else { 0 }
    }

    /// How many annotations at the front of a modifier list annotate the
    /// declaration: all of them, less any known type annotations that come
    /// right before the type.
    fn declaration_annotations(&self, modifiers: &SyntaxNode) -> usize {
        let elements: Vec<SyntaxElement> = significant_children(modifiers).collect();
        let leading = elements
            .iter()
            .take_while(|el| el.kind() == ANNOTATION)
            .count();
        if leading < elements.len() {
            return leading;
        }
        let type_use = elements
            .iter()
            .rev()
            .take_while(|el| self.is_type_annotation(el))
            .count();
        leading - type_use
    }

    /// Whether `annotation` is a marker annotation whose simple name was
    /// imported from a package of type annotations.
    fn is_type_annotation(&self, annotation: &SyntaxElement) -> bool {
        if annotation.kind() != ANNOTATION || has_arguments(annotation) {
            return false;
        }
        let name: String = annotation
            .to_string()
            .trim_start_matches('@')
            .split_whitespace()
            .collect();
        self.type_annotations.contains(&name)
    }

    fn modifiers(&self, node: &SyntaxNode) -> Doc {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let lines = self.annotation_lines(node);
        if lines == 0 {
            return self.spaced(&elements);
        }
        let mut docs = Vec::new();
        for annotation in &elements[..lines] {
            docs.push(self.element(annotation));
            docs.push(Doc::HardLine);
        }
        docs.push(self.spaced(&elements[lines..]));
        concat(docs)
    }

    /// `class Name<T> extends A implements B, C {`: each clause moves to a
    /// line of its own if it does not fit where it is, and the types it
    /// lists then wrap one per line.
    fn type_decl(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let is_tail = |kind: SyntaxKind| {
            matches!(
                kind,
                TYPE_PARAMS
                    | PARAM_LIST
                    | EXTENDS_CLAUSE
                    | IMPLEMENTS_CLAUSE
                    | PERMITS_CLAUSE
                    | CLASS_BODY
            )
        };
        let split = elements.iter().position(|el| is_tail(el.kind()))?;
        let mut docs = vec![self.spaced(&elements[..split])];
        // Type parameters, which the first clause is grouped with: if they
        // wrap, so does it.
        let mut before_clauses = Vec::new();
        for element in &elements[split..] {
            match element {
                NodeOrToken::Node(clause)
                    if matches!(
                        clause.kind(),
                        EXTENDS_CLAUSE | IMPLEMENTS_CLAUSE | PERMITS_CLAUSE
                    ) =>
                {
                    before_clauses.push(indent(
                        self.continuation_indent(),
                        concat(vec![Doc::Line, self.type_list_clause(clause)?]),
                    ));
                    docs.push(group(concat(std::mem::take(&mut before_clauses))));
                }
                NodeOrToken::Node(body) if body.kind() == CLASS_BODY => {
                    docs.append(&mut before_clauses);
                    docs.push(space());
                    docs.push(self.node(body));
                }
                // Record components wrap on their own: `implements` stays
                // after their `)` if it fits there.
                NodeOrToken::Node(components) if components.kind() == PARAM_LIST => {
                    docs.append(&mut before_clauses);
                    docs.push(self.node(components));
                }
                NodeOrToken::Node(node) if is_tail(node.kind()) => {
                    before_clauses.push(self.node(node));
                }
                _ => return None,
            }
        }
        docs.append(&mut before_clauses);
        Some(concat(docs))
    }

    /// `extends A, B` or `throws A, B`: a keyword and the types after it,
    /// which wrap one per line if they do not fit.
    fn type_list_clause(&self, clause: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(clause).collect();
        let (keyword, types) = elements.split_first()?;
        let (first, others) = types.split_first()?;
        Some(concat(vec![
            self.element(keyword),
            space(),
            group(concat(vec![
                self.element(first),
                indent(self.continuation_indent(), self.comma_separated(others)),
            ])),
        ]))
    }

    /// `<A, B>`: type arguments or parameters. If they do not fit they move
    /// to the next line, and if they still do not fit, to a line each.
    fn angle_list(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let (open, rest) = elements.split_first()?;
        let (close, items) = rest.split_last()?;
        if items.is_empty() {
            return Some(concat(vec![self.element(open), self.element(close)]));
        }
        let parent = node.parent()?;
        // `this.<T>f()`: the type arguments of a call stay with its name.
        if matches!(parent.kind(), METHOD_CALL | NEW_EXPR | METHOD_REF)
            && node.next_sibling_or_token().is_some_and(|next| {
                std::iter::successors(Some(next), |el| el.next_sibling_or_token())
                    .find(|el| !el.kind().is_trivia())
                    .is_some_and(|el| el.kind() == IDENT)
            })
        {
            return None;
        }
        // Parameters that are followed by more of the declaration's header
        // are indented twice, to set them apart from it.
        let double = node.kind() == TYPE_PARAMS
            && (matches!(parent.kind(), METHOD_DECL | CONSTRUCTOR_DECL)
                || parent.children().any(|sibling| {
                    matches!(
                        sibling.kind(),
                        EXTENDS_CLAUSE | IMPLEMENTS_CLAUSE | PERMITS_CLAUSE
                    )
                }));
        let columns = self.continuation_indent() * if double { 2 } else { 1 };
        Some(concat(vec![
            self.element(open),
            group(indent(
                columns,
                concat(vec![Doc::SoftLine, group(self.comma_separated(items))]),
            )),
            self.element(close),
        ]))
    }

    /// A method or constructor header.
    ///
    /// Three places may break, each on its own: after the type parameters,
    /// after the return type, and after `(`. Everything after a break that
    /// was taken is indented once more. If the header still does not fit,
    /// `throws` moves to its own line.
    fn method_decl(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let params = elements.iter().position(|el| el.kind() == PARAM_LIST)?;
        let name = params.checked_sub(1)?;
        if elements[name].kind() != IDENT {
            return None;
        }
        let mut before = &elements[..name];
        let mut docs = Vec::new();
        if let Some((modifiers, rest)) = before.split_first()
            && modifiers.kind() == MODIFIERS
        {
            docs.push(self.element(modifiers));
            docs.push(space());
            before = rest;
        }
        let (type_params, return_type) = match before {
            [] => (None, None),
            [only] if only.kind() == TYPE_PARAMS => (Some(only), None),
            [only] => (None, Some(only)),
            [first, second] if first.kind() == TYPE_PARAMS => (Some(first), Some(second)),
            _ => return None,
        };

        let continuation = self.continuation_indent();
        let one_per_line = self.tag();
        let mut tail = vec![
            self.element(&elements[name]),
            self.list(elements[params].as_node()?, Some(one_per_line))?,
        ];
        let mut body = Vec::new();
        for element in &elements[params + 1..] {
            match element {
                NodeOrToken::Token(token) if matches!(token.kind(), L_BRACK | R_BRACK) => {
                    tail.push(self.token(token));
                }
                // `throws` gets its own line if it does not fit after the
                // `)`, and always when the parameters take a line each.
                NodeOrToken::Node(throws) if throws.kind() == THROWS_CLAUSE => {
                    tail.push(group_if(
                        one_per_line,
                        indent(
                            continuation,
                            concat(vec![Doc::Line, self.type_list_clause(throws)?]),
                        ),
                    ));
                }
                NodeOrToken::Token(semi) if semi.kind() == SEMI => body.push(self.token(semi)),
                NodeOrToken::Node(node) if matches!(node.kind(), BLOCK | DEFAULT_VALUE) => {
                    body.push(space());
                    body.push(self.node(node));
                }
                _ => return None,
            }
        }

        let (after_params, after_type) = (self.tag(), self.tag());
        let type_group = return_type.map(|return_type| {
            let ty = if type_params.is_none() {
                // A comment between the modifiers and the type must not make
                // the name wrap.
                let (comments, ty) = self.split_comments(return_type);
                docs.extend(comments);
                ty
            } else {
                self.element(return_type)
            };
            tagged_group(
                after_type,
                concat(vec![ty, indent(continuation, Doc::Line)]),
            )
        });
        let tail = indent_if(after_type, continuation, concat(tail));
        let header = match type_params {
            Some(type_params) => {
                let mut inner = vec![Doc::Line];
                inner.extend(type_group);
                concat(vec![
                    tagged_group(
                        after_params,
                        concat(vec![
                            self.element(type_params),
                            indent(continuation, concat(inner)),
                        ]),
                    ),
                    indent_if(after_params, continuation, tail),
                ])
            }
            None => {
                let mut header: Vec<Doc> = type_group.into_iter().collect();
                header.push(tail);
                concat(header)
            }
        };
        docs.push(header);
        docs.extend(body);
        Some(concat(docs))
    }

    /// A field or local variable declaration. The name moves to the next
    /// line if the type leaves no room for it, and several variables wrap
    /// one per line.
    fn var_decl(&self, node: &SyntaxNode) -> Option<Doc> {
        let (declaration, semi, _) = self.var_decl_parts(node)?;
        Some(concat(vec![declaration, semi]))
    }

    /// Whether a field's annotations, written on the declaration's line,
    /// will move above it because the declaration is too long for one line.
    /// Such a field is set off by blank lines.
    fn field_wraps_annotations(&self, field: &SyntaxNode) -> bool {
        let first = first_token(field).map(|first| first.text_range().start());
        let previous = self.skip_leading.replace(first);
        let parts = self.var_decl_parts(field);
        self.skip_leading.set(previous);
        let Some((declaration, semi, true)) = parts else {
            return false;
        };
        // Fields sit in class bodies, so their indentation is known.
        let depth = field
            .ancestors()
            .filter(|node| matches!(node.kind(), CLASS_BODY | BLOCK | SWITCH_GROUP))
            .count();
        let indent = depth * self.block_indent() as usize;
        declaration.forces_break()
            || declaration.has_comment_break()
            || indent + declaration.flat_width() + semi.flat_width()
                > crate::FormatOptions::LINE_WIDTH
    }

    /// The parts of a variable declaration: the declaration, the `;` with
    /// its comments (or nothing), and whether the declaration starts with
    /// annotations that share its line.
    fn var_decl_parts(&self, node: &SyntaxNode) -> Option<(Doc, Doc, bool)> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let mut rest = elements.as_slice();
        let mut docs = Vec::new();
        // Annotations that share the declaration's line, which then move
        // above it together if the declaration does not fit on one line.
        let mut inline_annotations = None;
        if let Some((modifiers, after)) = rest.split_first()
            && modifiers.kind() == MODIFIERS
        {
            rest = after;
            let modifiers = modifiers.as_node()?;
            let parts: Vec<SyntaxElement> = significant_children(modifiers).collect();
            let declaration_annotations = self.declaration_annotations(modifiers);
            let in_foreach = node.parent().is_some_and(|p| p.kind() == FOREACH_STMT);
            if !in_foreach && self.annotation_lines(modifiers) == 0 && declaration_annotations > 0 {
                inline_annotations = Some(self.spaced(&parts[..declaration_annotations]));
                if parts.len() > declaration_annotations {
                    docs.push(self.spaced(&parts[declaration_annotations..]));
                    docs.push(space());
                }
            } else {
                docs.push(self.node(modifiers));
                docs.push(space());
            }
        }
        let (ty, mut rest) = rest.split_first()?;
        ty.as_node()?;
        let semi = match rest.split_last() {
            Some((semi, before)) if semi.kind() == SEMI => {
                rest = before;
                Some(semi)
            }
            _ => None,
        };
        let (first, others) = rest.split_first()?;
        if first.kind() != VARIABLE_DECLARATOR {
            return None;
        }

        let continuation = self.continuation_indent();
        let after_type = self.tag();
        let (comments, ty) = self.split_comments(ty);
        docs.extend(comments);
        let mut declaration = vec![tagged_group(
            after_type,
            concat(vec![
                indent(continuation, ty),
                indent(continuation, Doc::Line),
            ]),
        )];
        if others.is_empty() {
            declaration.push(indent_if(after_type, continuation, self.element(first)));
        } else {
            declaration.push(self.element(first));
            declaration.push(indent(continuation, self.comma_separated(others)));
        }
        docs.push(group(concat(declaration)));
        let semi = semi.map_or_else(|| concat(Vec::new()), |semi| self.element(semi));
        Some(match inline_annotations {
            Some(annotations) => (
                group(concat(vec![annotations, Doc::Line, concat(docs)])),
                semi,
                true,
            ),
            None => (concat(docs), semi, false),
        })
    }

    /// `A | B` in a `catch`: wraps before every `|` or before none.
    fn union_type(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let mut docs = Vec::new();
        for element in &elements {
            if element.kind() == PIPE {
                docs.push(Doc::Line);
                docs.push(self.element(element));
                docs.push(space());
            } else {
                docs.push(self.element(element));
            }
        }
        Some(group(indent(self.continuation_indent(), concat(docs))))
    }

    /// A parameter: the name moves to the next line if the type leaves no
    /// room for it. After modifiers, the lines the parameter goes on to are
    /// indented once more, and annotations in front of the type leave it
    /// the next line if the parameter does not fit on one.
    fn param(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let (name, before) = elements.split_last()?;
        let (ty, modifiers) = before.split_last()?;
        if name.kind() != IDENT || ty.as_node().is_none_or(|ty| ty.kind() == MODIFIERS) {
            return None;
        }
        let continuation = self.continuation_indent();
        let mut declaration = Vec::new();
        if ty.kind() == UNION_TYPE {
            declaration.push(self.element(ty));
            declaration.push(space());
        } else {
            declaration.push(group(concat(vec![
                indent(continuation, self.element(ty)),
                indent(continuation, Doc::Line),
            ])));
        }
        declaration.push(self.element(name));
        let modifiers = match modifiers {
            [] => return Some(concat(declaration)),
            [modifiers] if modifiers.kind() == MODIFIERS => modifiers.as_node()?,
            _ => return None,
        };
        let parts: Vec<SyntaxElement> = significant_children(modifiers).collect();
        // Annotations after a keyword, and known type annotations, belong
        // to the type and stay with it.
        let declaration_annotations = self.declaration_annotations(modifiers);
        let breaks = !parts.is_empty() && declaration_annotations == parts.len();
        let only_type_annotations =
            declaration_annotations == 0 && parts.iter().all(|part| part.kind() == ANNOTATION);
        if only_type_annotations {
            return Some(concat(vec![
                self.node(modifiers),
                space(),
                concat(declaration),
            ]));
        }
        // A comment above the parameter is not part of what wraps.
        let (mut docs, modifiers) = self.with_first_comments_apart(&parts, || self.node(modifiers));
        docs.push(group(indent(
            continuation,
            concat(vec![
                modifiers,
                if breaks { Doc::Line } else { space() },
                concat(declaration),
            ]),
        )));
        Some(concat(docs))
    }

    /// The braces of a class, interface, enum, record or annotation body.
    fn class_body(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let open = elements
            .first()?
            .as_token()
            .filter(|t| t.kind() == L_BRACE)?;
        let close = elements
            .last()?
            .as_token()
            .filter(|t| t.kind() == R_BRACE)?;
        let members = &elements[1..elements.len() - 1];
        if members.is_empty() && !self.has_dangling_comments(close) {
            return Some(concat(vec![self.token(open), self.token(close)]));
        }

        let in_enum = node.parent().is_some_and(|p| p.kind() == ENUM_DECL);
        let mut body = Vec::new();
        let mut previous: Option<SyntaxNode> = None;
        for member in members {
            let member = match member {
                // The `,` and `;` of enum constants stay on the constant's line.
                NodeOrToken::Token(token) => {
                    // After a trailing comma, the `;` gets its own line.
                    let after_comma = token.kind() == SEMI
                        && weavejig_syntax::prev_significant_token(token)
                            .is_some_and(|previous| previous.kind() == COMMA);
                    if previous.is_none() || after_comma {
                        body.push(if after_comma && self.blank_line_before(token) {
                            Doc::BlankLine
                        } else {
                            Doc::HardLine
                        });
                    }
                    body.push(self.token(token));
                    continue;
                }
                NodeOrToken::Node(member) => member,
            };
            // Enum constants, and fields without Javadoc or annotation lines,
            // may be packed together or spaced out as the author chose;
            // everything else gets a blank line.
            let separator = match previous.as_ref().map(|p| (p.kind(), p)) {
                // A blank line before the first enum constant is dropped,
                // unless a comment comes first.
                None if in_enum && !self.has_leading_comments(member) => Doc::HardLine,
                None => self.separator(member),
                Some((ENUM_CONSTANT, _)) if member.kind() == ENUM_CONSTANT => {
                    self.separator(member)
                }
                Some((ENUM_CONSTANT | FIELD_DECL, previous))
                    if member.kind() == FIELD_DECL
                        && !self.is_spaced_out(previous)
                        && !self.is_spaced_out(member) =>
                {
                    self.separator(member)
                }
                Some((EMPTY_DECL, _)) if member.kind() == EMPTY_DECL => self.separator(member),
                Some(_) => Doc::BlankLine,
            };
            body.push(separator);
            body.push(self.with_comments(&NodeOrToken::Node(member.clone())));
            previous = Some(member.clone());
        }
        self.dangling_comments(close, &mut body);
        Some(concat(vec![
            self.token(open),
            indent(self.block_indent(), concat(body)),
            Doc::HardLine,
            self.token_without_leading(close),
        ]))
    }

    /// Whether a field always has blank lines around it: one with Javadoc,
    /// or with annotations on lines of their own.
    fn is_spaced_out(&self, member: &SyntaxNode) -> bool {
        if member.kind() != FIELD_DECL {
            return false;
        }
        let javadoc = first_token(member)
            .and_then(|first| {
                self.trivia(&first)
                    .map(|t| t.leading.iter().any(Comment::is_javadoc))
            })
            .unwrap_or(false);
        javadoc
            || member
                .children()
                .find(|child| child.kind() == MODIFIERS)
                .is_some_and(|modifiers| self.annotation_lines(&modifiers) > 0)
            || self.field_wraps_annotations(member)
    }

    /// Whether an empty block is written `{}`. That is so for the body of a
    /// method, lambda or loop; a block that is one of several belonging to a
    /// statement (`if`/`else`, `try`/`catch`) is written with the braces on
    /// separate lines, as is a block standing on its own.
    fn empty_block_stays_closed(&self, block: &SyntaxNode) -> bool {
        let Some(parent) = block.parent() else {
            return false;
        };
        match parent.kind() {
            METHOD_DECL | CONSTRUCTOR_DECL | LAMBDA_EXPR | SWITCH_RULE | WHILE_STMT | FOR_STMT
            | FOREACH_STMT | DO_STMT => block.kind() == BLOCK,
            IF_STMT => {
                let has_else = parent.children_with_tokens().any(|el| el.kind() == ELSE_KW);
                let is_else_branch = parent.parent().is_some_and(|p| p.kind() == IF_STMT);
                !has_else && !is_else_branch
            }
            TRY_STMT => !parent
                .children()
                .any(|child| matches!(child.kind(), CATCH_CLAUSE | FINALLY_CLAUSE)),
            _ => false,
        }
    }

    // --- Statements --------------------------------------------------------

    /// `{ ... }` holding statements or switch entries, one per line. Blank
    /// lines between them are kept; blank lines at either end are dropped.
    fn block(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let open = elements
            .first()?
            .as_token()
            .filter(|t| t.kind() == L_BRACE)?;
        let close = elements
            .last()?
            .as_token()
            .filter(|t| t.kind() == R_BRACE)?;
        let statements = &elements[1..elements.len() - 1];
        if statements.is_empty() && !self.has_dangling_comments(close) {
            return Some(if self.empty_block_stays_closed(node) {
                concat(vec![self.token(open), self.token(close)])
            } else {
                concat(vec![self.token(open), Doc::HardLine, self.token(close)])
            });
        }
        // A blank line after the `{` is kept, except in a lambda or a switch.
        let keeps_first_blank =
            node.kind() == BLOCK && node.parent().is_none_or(|p| p.kind() != LAMBDA_EXPR);
        let mut body = Vec::new();
        for (i, statement) in statements.iter().enumerate() {
            match statement.as_node() {
                Some(node) if i > 0 || keeps_first_blank => body.push(self.separator(node)),
                _ => body.push(Doc::HardLine),
            }
            body.push(self.with_comments(statement));
        }
        // A blank line before the `}` is dropped, unless the statement goes
        // on after it: `} else {`, `} catch`, `} while (...)`.
        let continues = !self.has_dangling_comments(close)
            && self.blank_line_before(close)
            && weavejig_syntax::next_significant_token(close).is_some_and(|next| {
                match next.kind() {
                    ELSE_KW | CATCH_KW | FINALLY_KW => true,
                    WHILE_KW => node.parent().is_some_and(|parent| parent.kind() == DO_STMT),
                    _ => false,
                }
            });
        if node.kind() == SWITCH_BLOCK {
            let mut comments = Vec::new();
            self.dangling_comments(close, &mut comments);
            body.push(indent(self.block_indent(), concat(comments)));
        } else {
            self.dangling_comments(close, &mut body);
        }
        Some(concat(vec![
            self.token(open),
            indent(self.block_indent(), concat(body)),
            if continues {
                Doc::BlankLine
            } else {
                Doc::HardLine
            },
            self.token_without_leading(close),
        ]))
    }

    /// `case A:` labels, each on its own line, then the indented statements.
    fn switch_group(&self, node: &SyntaxNode) -> Doc {
        let mut labels = Vec::new();
        let mut body = Vec::new();
        for element in significant_children(node) {
            match &element {
                NodeOrToken::Node(label) if label.kind() == CASE_LABEL => {
                    if !labels.is_empty() {
                        labels.push(Doc::HardLine);
                    }
                    labels.push(self.node(label));
                }
                NodeOrToken::Node(statement) => {
                    body.push(if body.is_empty() {
                        Doc::HardLine
                    } else {
                        self.separator(statement)
                    });
                    body.push(self.with_comments(&element));
                }
                NodeOrToken::Token(token) => labels.push(self.token(token)),
            }
        }
        labels.push(indent(self.block_indent(), concat(body)));
        concat(labels)
    }

    /// `case A, B` or `case Type t when condition`: constants after the
    /// first wrap one per line, and a guard moves to a line of its own. The
    /// group that decides is tagged `wrapped`.
    fn case_label(&self, node: &SyntaxNode, wrapped: Tag) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let (keyword, rest) = elements.split_first()?;
        if rest.is_empty() {
            return None;
        }
        let mut first = Vec::new();
        let mut others = Vec::new();
        for (i, element) in rest.iter().enumerate() {
            let target = if others.is_empty() && element.kind() != GUARD {
                &mut first
            } else {
                &mut others
            };
            target.push(self.element(element));
            let next = rest.get(i + 1);
            // A line may break after each comma, and before a guard.
            if (element.kind() == COMMA && next.is_some())
                || next.is_some_and(|next| next.kind() == GUARD)
            {
                others.push(Doc::Line);
            }
        }
        Some(tagged_group(
            wrapped,
            concat(vec![
                self.element(keyword),
                space(),
                concat(first),
                indent(self.continuation_indent(), concat(others)),
            ]),
        ))
    }

    /// `case A -> value;`: a value that is not a block moves to the next
    /// line if it does not fit, and always when the label has wrapped.
    fn switch_rule(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let [label, arrow, body] = elements.as_slice() else {
            return None;
        };
        let label_node = label.as_node().filter(|label| label.kind() == CASE_LABEL)?;
        let wrapped = self.tag();
        let label = self
            .case_label(label_node, wrapped)
            .unwrap_or_else(|| self.generic(label_node));
        let mut docs = vec![label, space(), self.element(arrow)];
        if body.kind() == BLOCK {
            docs.push(space());
            docs.push(self.element(body));
        } else {
            docs.push(group_if(
                wrapped,
                indent(
                    self.continuation_indent(),
                    concat(vec![Doc::Line, self.element(body)]),
                ),
            ));
        }
        Some(concat(docs))
    }

    /// The body of an `if` or loop: a block stays on the same line, any
    /// other statement moves to the next line if it does not fit.
    fn body(&self, statement: &SyntaxElement) -> Doc {
        if statement.kind() == BLOCK {
            return concat(vec![space(), self.element(statement)]);
        }
        // An empty statement always gets the next line to itself.
        if statement.kind() == EMPTY_STMT {
            return indent(
                self.block_indent(),
                concat(vec![Doc::HardLine, self.element(statement)]),
            );
        }
        // `if (ready) // why`: google-java-format starts the statement on
        // the next line, at the `if`'s own indentation. The comment's line
        // break comes before the body, so the body is not indented.
        let after_comment = first_of(statement)
            .and_then(|first| weavejig_syntax::prev_significant_token(&first))
            .and_then(|previous| self.trivia(&previous))
            .is_some_and(|trivia| trivia.trailing.iter().any(|comment| comment.is_line));
        if after_comment {
            return self.element(statement);
        }
        group(indent(
            self.block_indent(),
            concat(vec![Doc::Line, self.element(statement)]),
        ))
    }

    fn if_stmt(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        // `if`, `(`, condition, `)`, then-branch, and optionally `else` and
        // the else-branch.
        if !matches!(elements.len(), 5 | 7) {
            return None;
        }
        let then = &elements[4];
        let mut docs = vec![self.spaced(&elements[..4]), self.body(then)];
        if let (Some(else_keyword), Some(otherwise)) = (elements.get(5), elements.get(6)) {
            docs.push(if then.kind() == BLOCK {
                space()
            } else {
                Doc::HardLine
            });
            docs.push(self.element(else_keyword));
            if otherwise.kind() == IF_STMT {
                docs.push(space());
                docs.push(self.element(otherwise));
            } else {
                docs.push(self.body(otherwise));
            }
        }
        Some(concat(docs))
    }

    /// `for (init; condition; update)`: the three parts wrap one per line.
    fn for_stmt(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let close = elements.iter().rposition(|el| el.kind() == R_PAREN)?;
        let [keyword, open, header @ ..] = &elements[..close] else {
            return None;
        };
        if open.kind() != L_PAREN || close + 2 != elements.len() {
            return None;
        }
        let mut inner = Vec::new();
        for (i, part) in header.split(|el| el.kind() == SEMI).enumerate() {
            if i > 0 {
                inner.push(text(";"));
                // `for (...; ...; )`: nothing to put on a line of its own.
                inner.push(if i == 2 && part.is_empty() {
                    space()
                } else {
                    Doc::Line
                });
            }
            inner.push(self.spaced(part));
        }
        // The `;` tokens were written by hand: they must carry no comments.
        let semis = header.iter().filter(|el| el.kind() == SEMI);
        let commented = |el: &SyntaxElement| {
            el.as_token()
                .and_then(|semi| self.trivia(semi))
                .is_some_and(|trivia| !trivia.leading.is_empty() || !trivia.trailing.is_empty())
        };
        if semis.clone().count() != 2 || semis.into_iter().any(commented) {
            return None;
        }
        Some(concat(vec![
            self.element(keyword),
            space(),
            self.element(open),
            group(indent(self.continuation_indent(), concat(inner))),
            self.element(&elements[close]),
            self.body(&elements[close + 1]),
        ]))
    }

    /// `for (Type name : values)`: the values move to the next line if they
    /// do not fit.
    fn foreach_stmt(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let [keyword, open, variable, colon, values, close, body] = elements.as_slice() else {
            return None;
        };
        if colon.kind() != COLON {
            return None;
        }
        Some(concat(vec![
            self.element(keyword),
            space(),
            self.element(open),
            self.element(variable),
            space(),
            self.element(colon),
            group(indent(
                self.continuation_indent(),
                concat(vec![Doc::Line, self.element(values)]),
            )),
            self.element(close),
            self.body(body),
        ]))
    }

    fn loop_stmt(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let (body, header) = elements.split_last()?;
        body.as_node()?;
        Some(concat(vec![self.spaced(header), self.body(body)]))
    }

    /// `do body while (condition);`: after a body that is not a block,
    /// `while` starts a line.
    fn do_stmt(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let [keyword, body, rest @ ..] = elements.as_slice() else {
            return None;
        };
        if keyword.kind() != DO_KW || rest.first()?.kind() != WHILE_KW {
            return None;
        }
        body.as_node()?;
        Some(concat(vec![
            self.element(keyword),
            self.body(body),
            if body.kind() == BLOCK {
                space()
            } else {
                Doc::HardLine
            },
            self.spaced(rest),
        ]))
    }

    /// `label:` on its own line, above the statement it names.
    fn labeled_stmt(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let (statement, label) = elements.split_last()?;
        Some(concat(vec![
            self.spaced(label),
            Doc::HardLine,
            self.element(statement),
        ]))
    }

    /// `try (a; b)`: resources after the first wrap one per line.
    fn resource_list(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let (open, rest) = elements.split_first()?;
        let (close, resources) = rest.split_last()?;
        let mut inner = Vec::new();
        for element in resources {
            if element.as_node().is_some() && !inner.is_empty() {
                inner.push(Doc::Line);
            }
            inner.push(self.element(element));
        }
        Some(concat(vec![
            self.element(open),
            group(indent(self.continuation_indent(), concat(inner))),
            self.element(close),
        ]))
    }

    /// `Type name = value` in a `try` header: the value moves to the next
    /// line, level with the other resources, if it does not fit.
    fn resource(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let equals = elements.iter().position(|el| el.kind() == EQ)?;
        let [value] = &elements[equals + 1..] else {
            return None;
        };
        // The value goes one indent in from the line the resource is on:
        // the `try` line for the first, a line of its own for the others.
        let first = node.prev_sibling().is_none();
        let columns = if first { 0 } else { self.continuation_indent() };
        Some(concat(vec![
            self.spaced(&elements[..=equals]),
            group(indent(
                columns,
                concat(vec![Doc::Line, self.element(value)]),
            )),
        ]))
    }

    // --- Expressions -------------------------------------------------------

    /// `target = value`, for assignments, variable initializers and
    /// annotation arguments: the value moves to the next line if the whole
    /// does not fit.
    fn assignment(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let operator = elements
            .iter()
            .position(|el| el.as_token().is_some_and(|t| t.kind().is_assignment_op()))?;
        let value = elements.get(operator + 1)?;
        if elements.len() != operator + 2 {
            return None;
        }
        let mut docs = vec![
            self.spaced(&elements[..operator]),
            space(),
            self.element(&elements[operator]),
        ];
        if value.kind() == ARRAY_INIT {
            // `int[] a = {` keeps the brace on the first line.
            docs.push(space());
            docs.push(self.element(value));
        } else {
            docs.push(group(indent(
                self.continuation_indent(),
                concat(vec![Doc::Line, self.element(value)]),
            )));
        }
        Some(concat(docs))
    }

    /// A chain of binary operators of one precedence, such as `a + b - c`,
    /// breaks before every operator or before none.
    fn binary(&self, node: &SyntaxNode) -> Option<Doc> {
        let precedence = binary_precedence(node)?;
        // The tree is left-nested; walk down the left spine iteratively so
        // that very long chains cannot overflow the stack.
        let mut tail = Vec::new();
        let mut current = node.clone();
        let first = loop {
            let elements: Vec<SyntaxElement> = significant_children(&current).collect();
            let (left, rest) = elements.split_first()?;
            let (right, operator) = rest.split_last()?;
            let left = left.as_node()?.clone();
            tail.push((operator.to_vec(), right.clone()));
            if left.kind() == BINARY_EXPR && binary_precedence(&left) == Some(precedence) {
                current = left;
            } else {
                break left;
            }
        };
        // As google-java-format does, operands that are all short fill
        // lines, `a + b + c` wrapping only where the next one does not fit.
        // Otherwise a wrapped expression breaks before every operator.
        let fills = self.is_short(&[NodeOrToken::Node(first.clone())])
            && tail
                .iter()
                .all(|(_, operand)| self.is_short(std::slice::from_ref(operand)));
        let mut rest = Vec::new();
        for (operator, operand) in tail.iter().rev() {
            // A shift is several adjacent `>` tokens.
            let mut item: Vec<Doc> = operator.iter().map(|token| self.element(token)).collect();
            item.push(space());
            item.push(self.element(operand));
            if fills {
                rest.push(fill_break(concat(item)));
            } else {
                rest.push(Doc::Line);
                rest.extend(item);
            }
        }
        // The first operand is indented too: if it wraps, its lines sit to
        // the right of the operators that follow it.
        let mut operands = vec![self.node(&first)];
        operands.extend(rest);
        Some(group(indent(self.continuation_indent(), concat(operands))))
    }

    fn conditional(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let [condition, question, then, colon, otherwise] = elements.as_slice() else {
            return None;
        };
        // The condition is indented too: if it wraps, its lines sit to the
        // right of the `?` and `:` that follow it.
        Some(group(indent(
            self.continuation_indent(),
            concat(vec![
                self.element(condition),
                Doc::Line,
                self.element(question),
                space(),
                self.element(then),
                Doc::Line,
                self.element(colon),
                space(),
                self.element(otherwise),
            ]),
        )))
    }

    /// A chain of field accesses and method calls, such as `a.b().c()`.
    ///
    /// The start of the chain, its *head*, never splits: `this.x`, a short
    /// name and what follows it (`map.get(k)`), or a class name and what
    /// follows it (`ImmutableList.builder()`, `System.out`). After the head:
    ///
    /// - A chain whose only call is its last link (`log.info(...)`,
    ///   `this.cache.get(...)`) keeps its dots together, and breaks before
    ///   the last one only if not even `.name(` fits.
    /// - Any other chain breaks before every remaining `.` or before none.
    ///
    /// Once a chain has wrapped, line breaks inside its head are indented
    /// once more, which sets the arguments of the first call apart from the
    /// links that follow.
    fn chain(&self, node: &SyntaxNode) -> Doc {
        // Each link is `.name` or `.name(args)`, innermost receiver last.
        let mut links: Vec<Vec<SyntaxElement>> = Vec::new();
        let mut current = node.clone();
        let (base_elements, base_node) = loop {
            let elements: Vec<SyntaxElement> = significant_children(&current).collect();
            let receiver = match (elements.first(), elements.get(1)) {
                (Some(NodeOrToken::Node(receiver)), Some(dot)) if dot.kind() == DOT => {
                    receiver.clone()
                }
                // A call without a receiver, such as `f(x)`.
                _ => break (elements, None),
            };
            links.push(elements[1..].to_vec());
            if matches!(receiver.kind(), METHOD_CALL | FIELD_ACCESS) {
                current = receiver;
            } else {
                break (vec![NodeOrToken::Node(receiver.clone())], Some(receiver));
            }
        };
        links.reverse();

        let is_call = |link: &Vec<SyntaxElement>| link.iter().any(|el| el.kind() == ARG_LIST);
        let name_of = |link: &Vec<SyntaxElement>| {
            link.iter()
                .find(|el| el.kind() == IDENT)
                .map(|el| el.to_string())
        };
        let base_kind = base_node.as_ref().map(|base| base.kind());
        // The name that follows a class name, if the chain ends with it:
        // `Constants.A_NAME`, `java.util.Map.Entry`.
        let mut after_class = None;
        let attached = match base_kind {
            Some(NAME_EXPR) => {
                let base_name = base_node.as_ref().map_or(String::new(), |b| b.to_string());
                let base_name = base_name.trim();
                // The base and the plain names after it: a class name among
                // them makes this a qualified name.
                let mut names = vec![base_name.to_string()];
                names.extend(
                    links
                        .iter()
                        .take_while(|link| !is_call(link))
                        .filter_map(name_of),
                );
                match names.iter().rposition(|name| is_class_name(name)) {
                    Some(class) => {
                        if class + 1 == links.len() && class + 2 == names.len() {
                            after_class = Some(class);
                        }
                        class + 1
                    }
                    None if base_name.chars().count() <= 4 => 1,
                    None => 0,
                }
            }
            // `this.x`, but not `Outer.this.x`.
            Some(THIS_EXPR | SUPER_EXPR) => usize::from(
                base_node
                    .as_ref()
                    .is_some_and(|base| significant_children(base).count() == 1),
            ),
            // `Type.class.getMethods()[0]`: a chain that is indexed keeps
            // the first call with the class literal.
            Some(CLASS_LITERAL) => usize::from(
                node.parent()
                    .is_some_and(|parent| parent.kind() == ARRAY_ACCESS),
            ),
            _ => 0,
        }
        .min(links.len());
        // The head also runs up to the last `.stream()` or `.toBuilder()`,
        // while that much fits on a line.
        let named = attached;
        let attached = links
            .iter()
            .rposition(|link| {
                is_call(link)
                    && name_of(link).is_some_and(|name| {
                        matches!(name.as_str(), "stream" | "parallelStream" | "toBuilder")
                    })
            })
            .map_or(attached, |last| attached.max(last + 1));
        // What follows the body of an anonymous class stays on its `}`: all
        // of it after a body with members, the first link after `{}`.
        let (named, attached) = match &base_node {
            Some(base) if base.children().any(|child| child.kind() == CLASS_BODY) => {
                let empty = base
                    .children()
                    .filter(|child| child.kind() == CLASS_BODY)
                    .all(|body| significant_children(&body).count() == 2);
                if empty {
                    (
                        named.max(1).min(links.len()),
                        attached.max(1).min(links.len()),
                    )
                } else {
                    (links.len(), links.len())
                }
            }
            _ => (named, attached),
        };

        // In the head of a chain that goes on, a block lambda is an ordinary
        // argument: `new Thread(() -> {` is not left hanging before `.start()`.
        let rest = &links[attached..];
        if !rest.is_empty() {
            let mut plain = self.plain_lists.borrow_mut();
            let base_children = base_node
                .iter()
                .flat_map(|base| base.children_with_tokens());
            for element in base_elements
                .iter()
                .cloned()
                .chain(base_children)
                .chain(links[..attached].iter().flatten().cloned())
            {
                if element.kind() == ARG_LIST {
                    plain.insert(element.text_range().start());
                }
            }
        }
        // The dot before a name that follows a class name gives way if not
        // even the name fits on the line: `Constants` and, under it,
        // `.A_VERY_LONG_NAME`.
        let attached_link = |(i, link): (usize, &Vec<SyntaxElement>)| {
            if after_class != Some(i) {
                return self.spaced(link);
            }
            group(indent(
                self.continuation_indent(),
                concat(vec![Doc::SoftLine, self.spaced(link)]),
            ))
        };
        let mut head = vec![self.spaced(&base_elements)];
        head.extend(links[..named].iter().enumerate().map(attached_link));
        // The head runs up to the last `.stream()` that it reaches on one
        // line. `with_streams` puts each `.stream()` in a group around the
        // head so far: the outermost group that fits stays whole, and the
        // links of those that do not fit take a line each.
        let streams: Vec<usize> = (named..attached)
            .filter(|&i| {
                is_call(&links[i])
                    && name_of(&links[i]).is_some_and(|name| {
                        matches!(name.as_str(), "stream" | "parallelStream" | "toBuilder")
                    })
            })
            .collect();
        let with_streams = |head: Doc| {
            let mut doc = head;
            let mut start = named;
            for &stream in &streams {
                let mut loose = Vec::new();
                for link in &links[start..=stream] {
                    loose.push(Doc::SoftLine);
                    loose.push(self.spaced(link));
                }
                doc = group(concat(vec![
                    doc,
                    indent(self.continuation_indent(), concat(loose)),
                ]));
                start = stream + 1;
            }
            // Past the last `.stream()` only what an anonymous class's body
            // holds on to can remain.
            let mut docs = vec![doc];
            docs.extend(
                links[start.min(attached)..attached]
                    .iter()
                    .map(|link| self.spaced(link)),
            );
            concat(docs)
        };
        let calls = links.iter().filter(|link| is_call(link)).count();
        // What an array access is made of is laid out on its own.
        let simple_base = matches!(
            base_kind,
            Some(NAME_EXPR | THIS_EXPR | SUPER_EXPR | ARRAY_ACCESS | CLASS_LITERAL)
        );
        let lone_call = simple_base && calls == 1 && links.last().is_some_and(is_call);
        let tag = self.tag();
        if lone_call && let Some((last, fields)) = links.split_last() {
            // Even a dot that belongs to the head gives way if `.name(`
            // does not fit after what comes before it.
            let mut head = vec![self.spaced(&base_elements)];
            head.extend(fields.iter().map(|link| self.spaced(link)));
            let arguments = last
                .iter()
                .position(|el| el.kind() == ARG_LIST)
                .unwrap_or(last.len());
            head.push(tagged_group(
                tag,
                indent(
                    self.continuation_indent(),
                    concat(vec![Doc::SoftLine, self.spaced(&last[..arguments])]),
                ),
            ));
            head.push(indent_if(
                tag,
                self.continuation_indent(),
                self.spaced(&last[arguments..]),
            ));
            return concat(head);
        }
        // `f(x).length` may wrap before its dot; `a.length` may not.
        if rest.is_empty() || (calls == 0 && rest.len() < 2 && simple_base) {
            let mut docs = vec![with_streams(concat(head))];
            docs.extend(rest.iter().map(|link| self.spaced(link)));
            return concat(docs);
        }

        let mut tail = Vec::new();
        for link in rest {
            tail.push(Doc::SoftLine);
            tail.push(self.spaced(link));
        }
        tagged_group(
            tag,
            concat(vec![
                with_streams(indent_if(tag, self.continuation_indent(), concat(head))),
                indent(self.continuation_indent(), concat(tail)),
            ]),
        )
    }

    /// `value instanceof Type`: wraps before `instanceof`, like a binary
    /// operator.
    fn instance_of(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let keyword = elements
            .iter()
            .position(|el| el.kind() == INSTANCEOF_KW)
            .filter(|keyword| *keyword == 1)?;
        Some(group(indent(
            self.continuation_indent(),
            concat(vec![
                self.element(&elements[0]),
                Doc::Line,
                self.spaced(&elements[keyword..]),
            ]),
        )))
    }

    /// `target::name`: wraps before the `::` if it does not fit.
    fn method_ref(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let colons = elements
            .iter()
            .position(|el| el.kind() == COLON_COLON)
            .filter(|colons| *colons == 1)?;
        Some(group(indent(
            self.continuation_indent(),
            concat(vec![
                self.element(&elements[0]),
                Doc::SoftLine,
                self.spaced(&elements[colons..]),
            ]),
        )))
    }

    /// `Type.class`: wraps before the `.` if not even that fits.
    fn class_literal(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let [ty, dot, class] = elements.as_slice() else {
            return None;
        };
        Some(group(concat(vec![
            self.element(ty),
            indent(
                self.continuation_indent(),
                concat(vec![Doc::SoftLine, self.element(dot), self.element(class)]),
            ),
        ])))
    }

    /// `T extends Bound`: the bound moves to the next line if it does not
    /// fit.
    fn type_param(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let (bound, name) = elements.split_last()?;
        let bound = bound.as_node().filter(|bound| bound.kind() == TYPE_BOUND)?;
        let parts: Vec<SyntaxElement> = significant_children(bound).collect();
        let (keyword, types) = parts.split_first()?;
        Some(concat(vec![
            self.spaced(name),
            space(),
            self.element(keyword),
            group(indent(
                self.continuation_indent(),
                concat(vec![Doc::Line, self.spaced(types)]),
            )),
        ]))
    }

    /// `(Type) value`: the value moves to the next line if it does not fit.
    fn cast(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let (value, cast) = elements.split_last()?;
        if cast.last()?.kind() != R_PAREN {
            return None;
        }
        Some(concat(vec![
            self.spaced(cast),
            group(indent(
                self.continuation_indent(),
                concat(vec![Doc::Line, self.element(value)]),
            )),
        ]))
    }

    /// A parenthesised, comma-separated list. If it does not fit after the
    /// `(`, it moves to the next line; if it still does not fit, every item
    /// gets its own line. `one_per_line`, if given, tags that last step.
    ///
    /// Arguments have three more layouts:
    ///
    /// - a lambda that is the only argument starts on the line of the call;
    /// - arguments written two to a line, as the keys and values of
    ///   `ImmutableMap.of`, stay that way (see [`Formatter::table_width`]);
    /// - short arguments fill lines instead of taking one each;
    /// - the arguments after a format string wrap apart from it: all on
    ///   the line after it, or one per line.
    fn list(&self, node: &SyntaxNode, one_per_line: Option<Tag>) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let (open, rest) = elements.split_first()?;
        let (close, items) = rest.split_last()?;
        if items.is_empty() {
            return Some(concat(vec![self.element(open), self.element(close)]));
        }
        let continuation = self.continuation_indent();
        let wrapped = |build: &dyn Fn() -> Doc| {
            let (mut before, inner) = self.with_first_comments_apart(items, build);
            let inner = match one_per_line {
                Some(tag) => tagged_group(tag, inner),
                None => group(inner),
            };
            before.insert(0, Doc::SoftLine);
            before.push(inner);
            // The `)` is inside the indentation, as in google-java-format:
            // after `f(a, b // why`, it starts a line indented like `a`. And
            // since the group encloses it, that line break breaks the group.
            before.push(self.element(close));
            concat(vec![
                self.element(open),
                group(indent(continuation, concat(before))),
            ])
        };
        // `@A(x = {1, 2}, y = 3)`: several named values, one of them an
        // array, take a line each.
        let pairs: Vec<&SyntaxElement> = items
            .iter()
            .filter(|item| item.kind() == ANNOTATION_PAIR)
            .collect();
        let has_array = |pair: &&SyntaxElement| {
            pair.as_node()
                .is_some_and(|pair| pair.children().any(|child| child.kind() == ARRAY_INIT))
        };
        if node.kind() == ANNOTATION_ARGS && pairs.len() > 1 && pairs.iter().any(has_array) {
            let mut inner = Vec::new();
            for value in items.split_inclusive(|el| el.kind() == COMMA) {
                inner.push(Doc::HardLine);
                inner.push(self.spaced(value));
            }
            return Some(concat(vec![
                self.element(open),
                indent(continuation, concat(inner)),
                self.element(close),
            ]));
        }
        // `@SuppressWarnings({"a", "b"})`: the array's braces stay with the
        // parentheses.
        if let [NodeOrToken::Node(only)] = items
            && node.kind() == ANNOTATION_ARGS
            && only.kind() == ARRAY_INIT
        {
            return Some(concat(vec![
                self.element(open),
                self.node(only),
                self.element(close),
            ]));
        }
        // `@A(x = 1, y = 2)`: several named values that do not fit after
        // the `(` take a line each.
        if node.kind() == ANNOTATION_ARGS && pairs.len() > 1 {
            let (mut inner, values) =
                self.with_first_comments_apart(items, || self.comma_separated(items));
            inner.insert(0, Doc::SoftLine);
            inner.push(values);
            inner.push(self.element(close));
            return Some(concat(vec![
                self.element(open),
                group(indent(continuation, concat(inner))),
            ]));
        }
        if node.kind() != ARG_LIST {
            return Some(wrapped(&|| self.comma_separated(items)));
        }

        // A comment after the `(` puts every argument on a line of its own.
        let open_commented = open
            .as_token()
            .and_then(|open| self.trivia(open))
            .is_some_and(|trivia| trivia.trailing.iter().any(|comment| comment.is_line));
        if let [NodeOrToken::Node(only)] = items
            && only.kind() == LAMBDA_EXPR
            && !open_commented
        {
            let block = only
                .children()
                .last()
                .is_some_and(|body| body.kind() == BLOCK);
            let plain = self
                .plain_lists
                .borrow()
                .contains(&node.text_range().start());
            if !block {
                let lambda = self.node(only);
                // A comment after the lambda puts the `)` on a line of its
                // own, and then the lambda is an ordinary argument.
                let commented = last_token(only)
                    .and_then(|last| {
                        self.trivia(&last)
                            .map(|t| t.trailing.iter().any(|c| c.is_line))
                    })
                    .unwrap_or(false);
                if commented {
                    return Some(wrapped(&|| self.comma_separated(items)));
                }
                // `f(() -> value)`, or the lambda on the next line if it
                // fits there, or `f(() ->` with the value below.
                return Some(concat(vec![
                    self.element(open),
                    next_line_if_fits(continuation, lambda),
                    self.element(close),
                ]));
            }
            if !plain {
                return Some(concat(vec![
                    self.element(open),
                    self.node(only),
                    self.element(close),
                ]));
            }
        }
        let arguments: Vec<&[SyntaxElement]> =
            items.split_inclusive(|el| el.kind() == COMMA).collect();
        // Arguments keep their rows only as pairs, with none left over.
        if arguments.len().is_multiple_of(2) && self.table_width(&arguments) == Some(2) {
            let mut inner = Vec::new();
            for pair in arguments.chunks(2) {
                inner.push(Doc::HardLine);
                let (comments, key) = self.item_with_comments_apart(pair[0]);
                inner.extend(comments);
                let mut row = vec![key];
                if let Some(value) = pair.get(1) {
                    row.push(indent(
                        continuation,
                        concat(vec![Doc::Line, self.spaced(value)]),
                    ));
                }
                inner.push(group(concat(row)));
            }
            // As in `wrapped`, the `)` is indented like the arguments if a
            // trailing comment puts it on a line of its own.
            inner.push(self.element(close));
            return Some(concat(vec![
                self.element(open),
                indent(continuation, concat(inner)),
            ]));
        }
        if self.fills(&arguments) {
            return Some(wrapped(&|| self.filled(&arguments)));
        }
        if let Some((first, others)) = arguments.split_first()
            && others.len() > 1
            && is_format_string(first)
        {
            let rest: Vec<SyntaxElement> = others
                .iter()
                .flat_map(|item| item.iter().cloned())
                .collect();
            return Some(wrapped(&|| {
                concat(vec![
                    self.spaced(first),
                    Doc::Line,
                    group(self.comma_separated(&rest)),
                ])
            }));
        }
        Some(wrapped(&|| self.comma_separated(items)))
    }

    /// Items, each with its comma, that fill lines: a line break goes only
    /// where the next item no longer fits.
    fn filled(&self, items: &[&[SyntaxElement]]) -> Doc {
        let mut docs = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let item = self.spaced(item);
            docs.push(if i == 0 { item } else { fill_break(item) });
        }
        concat(docs)
    }

    /// Whether an item of a list is short enough for the list to be filled:
    /// under ten characters as printed, with no comment inside it or
    /// between it and its comma.
    fn is_short(&self, item: &[SyntaxElement]) -> bool {
        let Some(value) = item.first() else {
            return false;
        };
        let tokens: Vec<SyntaxToken> = match value {
            NodeOrToken::Node(node) => node
                .descendants_with_tokens()
                .filter_map(|el| el.into_token())
                .filter(|token| !token.kind().is_trivia())
                .collect(),
            NodeOrToken::Token(token) => vec![token.clone()],
        };
        let last = tokens.len().saturating_sub(1);
        let commented = tokens.iter().enumerate().any(|(i, token)| {
            self.trivia(token).is_some_and(|t| {
                (i > 0 && !t.leading.is_empty()) || (i < last && !t.trailing.is_empty())
            })
        }) || item[1..]
            .iter()
            .filter_map(|el| el.as_token())
            .any(|comma| self.trivia(comma).is_some_and(|t| !t.leading.is_empty()));
        let text = value.to_string();
        let width: usize = text
            .split_whitespace()
            .map(|word| word.chars().count() + 1)
            .sum();
        let text_block = tokens.iter().any(|token| token.kind() == TEXT_BLOCK);
        !commented && !text_block && width.saturating_sub(1) < 10
    }

    /// Whether the items of a list fill lines rather than take one each:
    /// when all are short, and no comment has a line to itself among them.
    fn fills(&self, items: &[&[SyntaxElement]]) -> bool {
        items.iter().enumerate().all(|(i, item)| {
            let own_line_comment = item
                .first()
                .and_then(first_of)
                .is_some_and(|first| self.trivia(&first).is_some_and(|t| !t.leading.is_empty()));
            // A comment after the last item counts as part of it: a long
            // one makes the item long, and then every item gets a line.
            let comment_after_last =
                if i + 1 == items.len() && item.last().is_some_and(|last| last.kind() != COMMA) {
                    item.iter()
                        .filter_map(last_of)
                        .filter_map(|last| self.trivia(&last))
                        .flat_map(|trivia| trivia.trailing.iter())
                        .map(|comment| comment.text.trim_end().chars().count() + 1)
                        .sum::<usize>()
                } else {
                    0
                };
            let trailing_comment = comment_after_last >= 10;
            self.is_short(item) && (i == 0 || !own_line_comment) && !trailing_comment
        })
    }

    /// Builds the items of a list with `build`, having taken out the
    /// comments in front of the first item: they are returned apart, to be
    /// printed where they do not make the items take a line each.
    fn with_first_comments_apart(
        &self,
        items: &[SyntaxElement],
        build: impl FnOnce() -> Doc,
    ) -> (Vec<Doc>, Doc) {
        let first = items.first().and_then(first_of);
        let trivia = first
            .as_ref()
            .and_then(|first| self.trivia(first))
            .filter(|trivia| !trivia.leading.is_empty());
        let (Some(first), Some(trivia)) = (first.as_ref(), trivia) else {
            return (Vec::new(), build());
        };
        let start = first.text_range().start();
        if self.skip_leading.get() == Some(start) {
            return (Vec::new(), build());
        }
        let mut comments = Vec::new();
        self.leading_comments_spaced(trivia, Some(first.kind()), true, &mut comments);
        let previous = self.skip_leading.replace(Some(start));
        let doc = build();
        self.skip_leading.set(previous);
        (comments, doc)
    }

    /// How many items the source puts on each line of a list that it
    /// writes as a table, or `None` if it is not written as one.
    ///
    /// As google-java-format sees it, a table has at least two rows. Each
    /// row starts in the column of the first item, on a line after the
    /// opening bracket, and nothing starts to the left of that column. All
    /// rows are as long as the first, except that the last of three or more
    /// may be shorter. And the columns hold like things: the first items of
    /// all rows are one kind of expression, and in every other column more
    /// than half of the rows agree on one.
    fn table_width(&self, items: &[&[SyntaxElement]]) -> Option<usize> {
        let mut rows: Vec<Vec<&SyntaxElement>> = Vec::new();
        let mut margin = 0;
        for (i, item) in items.iter().enumerate() {
            let value = item.first()?;
            let column = self.source_column(&first_of(value)?);
            if i == 0 {
                margin = column;
            }
            match column.cmp(&margin) {
                std::cmp::Ordering::Less => return None,
                std::cmp::Ordering::Equal => rows.push(vec![value]),
                std::cmp::Ordering::Greater => rows.last_mut()?.push(value),
            }
        }
        let width = rows.first()?.len();
        let (last, full) = rows.split_last()?;
        let regular = match full.len() {
            0 => false,
            1 => last.len() == width,
            _ => full.iter().all(|row| row.len() == width) && last.len() <= width,
        };
        if !regular {
            return None;
        }
        let agree = |column: usize, at_least: usize| {
            let mut kinds: Vec<String> = rows
                .iter()
                .filter_map(|row| row.get(column))
                .map(|value| expression_kind(value))
                .collect();
            kinds.sort();
            kinds
                .chunk_by(|a, b| a == b)
                .any(|same| same.len() >= at_least)
        };
        (agree(0, rows.len()) && (1..width).all(|column| agree(column, rows.len() / 2 + 1)))
            .then_some(width)
    }

    /// An item of a list, with its comma, and apart from it the comments in
    /// front of it: for a caller that groups the item, which comments on
    /// lines of their own must not break.
    fn item_with_comments_apart(&self, item: &[SyntaxElement]) -> (Vec<Doc>, Doc) {
        self.with_first_comments_apart(item, || self.spaced(item))
    }

    fn comma_separated(&self, items: &[SyntaxElement]) -> Doc {
        let mut docs = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let commented =
                |token: &SyntaxToken| self.trivia(token).is_some_and(|t| !t.leading.is_empty());
            if i > 0 && item.as_token().is_some_and(commented) {
                docs.push(space());
            }
            docs.push(self.element(item));
            if item.kind() == COMMA && i + 1 < items.len() {
                docs.push(Doc::Line);
            }
        }
        concat(docs)
    }

    /// `{a, b}`, or with the braces on their own lines if it does not fit.
    /// Values written as a table (see [`Formatter::table_width`]) keep
    /// their rows; a row that is too long fills further, indented lines.
    fn array_init(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let (open, rest) = elements.split_first()?;
        let (close, items) = rest.split_last()?;
        let close = close.as_token()?;
        if items.is_empty() && !self.has_dangling_comments(close) {
            return Some(concat(vec![self.element(open), self.token(close)]));
        }
        let values: Vec<&[SyntaxElement]> =
            items.split_inclusive(|el| el.kind() == COMMA).collect();
        let trailing_comma = items.last().is_some_and(|last| last.kind() == COMMA);
        // A table one value wide is one only if its rows are arrays of
        // their own, of two values or more.
        let is_row = |value: &&[SyntaxElement]| {
            value
                .first()
                .and_then(|value| value.as_node())
                .is_some_and(|value| {
                    std::iter::once(value.clone())
                        .chain(value.children())
                        .find(|node| node.kind() == ARRAY_INIT)
                        .filter(|_| matches!(value.kind(), ARRAY_INIT | ARRAY_CREATION_EXPR))
                        .is_some_and(|rows| {
                            rows.children_with_tokens().any(|el| el.kind() == COMMA)
                        })
                })
        };
        let table = self
            .table_width(&values)
            .filter(|width| *width > 1 || values.iter().all(is_row));
        if let Some(width) = table {
            let mut inner = Vec::new();
            for row in values.chunks(width) {
                inner.push(Doc::HardLine);
                let (comments, first) = self.item_with_comments_apart(row[0]);
                inner.extend(comments);
                let rest = row[1..]
                    .iter()
                    .map(|item| fill_break(self.spaced(item)))
                    .collect();
                inner.push(group(concat(vec![
                    first,
                    indent(self.continuation_indent(), concat(rest)),
                ])));
            }
            self.dangling_comments(close, &mut inner);
            return Some(concat(vec![
                self.element(open),
                indent(self.block_indent(), concat(inner)),
                Doc::HardLine,
                self.token_without_leading(close),
            ]));
        }
        // A trailing comma puts the braces on lines of their own.
        let first = if trailing_comma {
            Doc::HardLine
        } else {
            Doc::SoftLine
        };
        // In an annotation, values that do not fill lines wrap with the
        // braces: one per line as soon as the braces are on their own.
        let in_annotation = node
            .parent()
            .is_some_and(|parent| matches!(parent.kind(), ANNOTATION_ARGS | ANNOTATION_PAIR));
        let mut inner = if self.fills(&values) {
            vec![first, group(self.filled(&values))]
        } else if in_annotation {
            vec![first, self.comma_separated(items)]
        } else {
            vec![first, group(self.comma_separated(items))]
        };
        self.dangling_comments(close, &mut inner);
        Some(group(concat(vec![
            self.element(open),
            indent(self.block_indent(), concat(inner)),
            Doc::SoftLine,
            self.token_without_leading(close),
        ])))
    }

    fn lambda(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let (body, head) = elements.split_last()?;
        if body.kind() == BLOCK {
            return None;
        }
        Some(concat(vec![
            self.spaced(head),
            group(indent(
                self.continuation_indent(),
                concat(vec![Doc::Line, self.element(body)]),
            )),
        ]))
    }
}

/// What kind of expression an item of a list is, as far as lining up the
/// columns of a table goes: `1` and `-2` are alike, `1` and `"a"` are not,
/// and neither are `a + b` and `a - b`.
fn expression_kind(element: &SyntaxElement) -> String {
    let NodeOrToken::Node(node) = element else {
        return format!("{:?}", element.kind());
    };
    let tokens = || {
        significant_children(node)
            .filter_map(|el| el.into_token())
            .map(|token| token.text().to_string())
            .collect::<String>()
    };
    match node.kind() {
        LITERAL => {
            let Some(token) = first_token(node) else {
                return "literal".to_string();
            };
            let text = token.text();
            match token.kind() {
                INT_LITERAL if text.ends_with(['l', 'L']) => "long",
                INT_LITERAL => "int",
                FLOAT_LITERAL if text.ends_with(['f', 'F']) => "float",
                FLOAT_LITERAL => "double",
                CHAR_LITERAL => "char",
                STRING_LITERAL | TEXT_BLOCK => "string",
                NULL_KW => "null",
                _ => "boolean",
            }
            .to_string()
        }
        PREFIX_EXPR => {
            let operand = node.children().next();
            let number = operand.as_ref().filter(|operand| {
                operand.kind() == LITERAL
                    && first_token(operand)
                        .is_some_and(|token| matches!(token.kind(), INT_LITERAL | FLOAT_LITERAL))
            });
            // A negative number is a number.
            match number {
                Some(number) if tokens() == "-" => {
                    expression_kind(&NodeOrToken::Node(number.clone()))
                }
                _ => format!("prefix {}", tokens()),
            }
        }
        POSTFIX_EXPR => format!("postfix {}", tokens()),
        BINARY_EXPR => format!("binary {}", tokens()),
        ASSIGN_EXPR => format!("assign {}", tokens()),
        FIELD_ACCESS | CLASS_LITERAL => "select".to_string(),
        NAME_EXPR | THIS_EXPR | SUPER_EXPR => "name".to_string(),
        ARRAY_CREATION_EXPR | ARRAY_INIT => "array".to_string(),
        kind => format!("{kind:?}"),
    }
}

/// Whether a node is an expression that has a layout of its own.
fn is_expression(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        BINARY_EXPR
            | CONDITIONAL_EXPR
            | METHOD_CALL
            | FIELD_ACCESS
            | CAST_EXPR
            | INSTANCEOF_EXPR
            | ASSIGN_EXPR
            | LAMBDA_EXPR
            | METHOD_REF
            | NEW_EXPR
            | ARRAY_CREATION_EXPR
            | ARRAY_INIT
            | ARRAY_ACCESS
            | CLASS_LITERAL
            | SWITCH_EXPR
    )
}

/// Whether an argument is a format string: a string literal or text block
/// with a `%` or a `{0}` in it, or a concatenation of string literals with
/// one.
fn is_format_string(argument: &[SyntaxElement]) -> bool {
    let Some(NodeOrToken::Node(value)) = argument.first() else {
        return false;
    };
    let tokens: Vec<SyntaxToken> = value
        .descendants_with_tokens()
        .filter_map(|el| el.into_token())
        .filter(|token| !token.kind().is_trivia())
        .collect();
    let placeholder = |text: &str| {
        text.contains('%')
            || text
                .as_bytes()
                .windows(3)
                .any(|window| window[0] == b'{' && window[1].is_ascii_digit() && window[2] == b'}')
    };
    tokens
        .iter()
        .all(|token| matches!(token.kind(), STRING_LITERAL | TEXT_BLOCK | PLUS))
        && tokens.iter().any(|token| placeholder(token.text()))
}

/// Whether a comment's text is exactly `$NON-NLS-1$`, with any number.
fn is_non_nls_marker(body: &str) -> bool {
    body.strip_prefix("$NON-NLS-")
        .and_then(|rest| rest.strip_suffix('$'))
        .is_some_and(|number| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether an identifier looks like a class name: `Foo` or `T`, but not
/// `foo` or `MAX_VALUE`.
fn is_class_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(char::is_uppercase)
        && (chars.clone().next().is_none() || chars.any(char::is_lowercase))
}

/// The precedence class of a binary expression's operator. Operators in one
/// class (`+` and `-`, say) wrap together as one chain.
fn binary_precedence(node: &SyntaxNode) -> Option<u8> {
    let operator: String = significant_children(node)
        .filter_map(|el| el.into_token())
        .map(|token| token.text().to_string())
        .collect();
    Some(match operator.as_str() {
        "||" => 1,
        "&&" => 2,
        "|" => 3,
        "^" => 4,
        "&" => 5,
        "==" | "!=" => 6,
        "<" | ">" | "<=" | ">=" => 7,
        "<<" | ">>" | ">>>" => 8,
        "+" | "-" => 9,
        "*" | "/" | "%" => 10,
        _ => return None,
    })
}
