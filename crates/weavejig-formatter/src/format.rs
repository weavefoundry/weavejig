//! Formatting rules: syntax tree in, layout document out.
//!
//! [`Formatter::node`] dispatches on the node kind. Kinds without a rule of
//! their own fall back to [`Formatter::generic`], which lays the node's
//! children out on one line using the spacing table. A rule exists wherever
//! a construct needs line breaks or indentation.

use std::cell::Cell;
use std::collections::HashSet;

use weavejig_syntax::{
    NodeOrToken, SyntaxElement, SyntaxKind, SyntaxKind::*, SyntaxNode, SyntaxToken, TextSize,
    first_token, last_token, significant_children,
};

use crate::FormatOptions;
use crate::comments::{Comment, Comments, TokenTrivia};
use crate::doc::{Doc, Tag, concat, group, indent, indent_if, space, tagged_group, text};
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
}

/// Annotations that google-java-format keeps next to the type they annotate
/// when they are imported by name.
const TYPE_ANNOTATIONS: [&str; 4] = [
    "org.jspecify.annotations.Nullable",
    "org.jspecify.annotations.NonNull",
    "org.checkerframework.checker.nullness.qual.Nullable",
    "org.checkerframework.checker.nullness.qual.NonNull",
];

/// The dotted name an import declaration names, and whether it is static.
fn import_name(import: &SyntaxNode) -> (bool, String) {
    let mut is_static = false;
    let mut name = String::new();
    for element in significant_children(import) {
        match element.kind() {
            IMPORT_KW | SEMI => {}
            STATIC_KW => is_static = true,
            _ => name.push_str(&element.to_string()),
        }
    }
    (is_static, name.split_whitespace().collect())
}

fn has_arguments(annotation: &SyntaxElement) -> bool {
    annotation
        .as_node()
        .is_some_and(|a| a.children().any(|c| c.kind() == ANNOTATION_ARGS))
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
        let line = comment.text.trim_end().to_string();
        return if trailing {
            Doc::Suffix(line)
        } else {
            text(line)
        };
    }
    let lines: Vec<&str> = comment.text.lines().collect();
    if lines.len() < 2 {
        return text(comment.text.clone());
    }
    // Javadoc-style comments, where every line starts with `*`, are
    // re-indented so the stars line up. Anything else is kept verbatim.
    if lines[1..]
        .iter()
        .all(|line| line.trim_start().starts_with('*'))
    {
        let mut docs = vec![text(lines[0].trim_end())];
        for line in &lines[1..] {
            docs.push(Doc::HardLine);
            docs.push(text(format!(" {}", line.trim())));
        }
        return concat(docs);
    }
    text(
        lines
            .iter()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n"),
    )
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
        Formatter {
            comments: Comments::collect(root),
            options,
            skip_leading: Cell::new(None),
            type_annotations,
            next_tag: Cell::new(0),
        }
    }

    fn tag(&self) -> Tag {
        let tag = self.next_tag.get();
        self.next_tag.set(tag + 1);
        tag
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
        let Some(trivia) = self.trivia(token) else {
            return text(token.text());
        };
        let mut docs = Vec::new();
        if self.skip_leading.get() != Some(token.text_range().start()) {
            self.leading_comments(trivia, Some(token.kind()), &mut docs);
        }
        docs.push(text(token.text()));
        for comment in &trivia.trailing {
            docs.push(space());
            docs.push(comment_doc(comment, true));
            if comment.is_line {
                docs.push(Doc::HardLine);
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
        for (i, comment) in trivia.leading.iter().enumerate() {
            if comment.newlines_before > 0 {
                // A blank line before the first comment is the caller's call.
                let blank = i > 0 && comment.newlines_before >= 2;
                docs.push(if blank { Doc::BlankLine } else { Doc::HardLine });
            }
            docs.push(comment_doc(comment, false));
            let next = trivia.leading.get(i + 1);
            let newlines_after = next.map_or(trivia.newlines_before, |c| c.newlines_before);
            if next.is_some() {
                if newlines_after == 0 {
                    docs.push(space());
                }
            } else if newlines_after >= 2 {
                docs.push(Doc::BlankLine);
            } else if newlines_after == 1 || comment.is_line {
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

    fn has_dangling_comments(&self, close: &SyntaxToken) -> bool {
        self.trivia(close).is_some_and(|t| !t.leading.is_empty())
    }

    /// A statement or declaration with the comments in front of it printed
    /// first, outside the element's own layout. A comment on its own line
    /// must not make the code after it wrap.
    fn with_comments(&self, element: &SyntaxElement) -> Doc {
        let Some(first) = first_of(element) else {
            return self.element(element);
        };
        let Some(trivia) = self.trivia(&first).filter(|t| !t.leading.is_empty()) else {
            return self.element(element);
        };
        let mut docs = Vec::new();
        self.leading_comments(trivia, Some(first.kind()), &mut docs);
        let previous = self.skip_leading.replace(Some(first.text_range().start()));
        docs.push(self.element(element));
        self.skip_leading.set(previous);
        concat(docs)
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
        let doc = match node.kind() {
            COMPILATION_UNIT => Some(self.compilation_unit(node)),
            MODULE_DECL => self.module_decl(node),
            MODIFIERS => Some(self.modifiers(node)),
            CLASS_BODY => self.class_body(node),
            BLOCK | SWITCH_BLOCK => self.block(node),
            SWITCH_GROUP => Some(self.switch_group(node)),
            IF_STMT => self.if_stmt(node),
            WHILE_STMT | FOR_STMT | FOREACH_STMT => self.loop_stmt(node),
            LABELED_STMT => self.labeled_stmt(node),
            RESOURCE_LIST => self.resource_list(node),
            VARIABLE_DECLARATOR | ASSIGN_EXPR | ANNOTATION_PAIR => self.assignment(node),
            BINARY_EXPR => self.binary(node),
            CONDITIONAL_EXPR => self.conditional(node),
            METHOD_CALL | FIELD_ACCESS => Some(self.chain(node)),
            ARG_LIST | PARAM_LIST | LAMBDA_PARAMS | ANNOTATION_ARGS => self.list(node),
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
            docs.push(if changed_block {
                Doc::BlankLine
            } else {
                Doc::HardLine
            });
            docs.push(self.generic(import));
            previous_static = Some(non_static);
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
        let mut body = Vec::new();
        for directive in elements[open + 1..elements.len() - 1].iter() {
            body.push(
                directive
                    .as_node()
                    .map_or(Doc::HardLine, |node| self.separator(node)),
            );
            body.push(self.element(directive));
        }
        self.dangling_comments(close, &mut body);
        Some(concat(vec![
            self.spaced(&elements[..=open]),
            indent(self.block_indent(), concat(body)),
            Doc::HardLine,
            self.token_without_leading(close),
        ]))
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
                    if previous.is_none() {
                        body.push(Doc::HardLine);
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
                // A blank line before the first enum constant is dropped.
                None if in_enum => Doc::HardLine,
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
        self.dangling_comments(close, &mut body);
        Some(concat(vec![
            self.token(open),
            indent(self.block_indent(), concat(body)),
            Doc::HardLine,
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

    /// The body of an `if` or loop: a block stays on the same line, any
    /// other statement moves to the next line if it does not fit.
    fn body(&self, statement: &SyntaxElement) -> Doc {
        if statement.kind() == BLOCK {
            return concat(vec![space(), self.element(statement)]);
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

    fn loop_stmt(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let (body, header) = elements.split_last()?;
        body.as_node()?;
        Some(concat(vec![self.spaced(header), self.body(body)]))
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
        let mut rest = Vec::new();
        for (operator, operand) in tail.iter().rev() {
            rest.push(Doc::Line);
            // A shift is several adjacent `>` tokens.
            rest.extend(operator.iter().map(|token| self.element(token)));
            rest.push(space());
            rest.push(self.element(operand));
        }
        Some(group(concat(vec![
            self.node(&first),
            indent(self.continuation_indent(), concat(rest)),
        ])))
    }

    fn conditional(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let [condition, question, then, colon, otherwise] = elements.as_slice() else {
            return None;
        };
        Some(group(concat(vec![
            self.element(condition),
            indent(
                self.continuation_indent(),
                concat(vec![
                    Doc::Line,
                    self.element(question),
                    space(),
                    self.element(then),
                    Doc::Line,
                    self.element(colon),
                    space(),
                    self.element(otherwise),
                ]),
            ),
        ])))
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
        let (base, base_node) = loop {
            let elements: Vec<SyntaxElement> = significant_children(&current).collect();
            let receiver = match (elements.first(), elements.get(1)) {
                (Some(NodeOrToken::Node(receiver)), Some(dot)) if dot.kind() == DOT => {
                    receiver.clone()
                }
                // A call without a receiver, such as `f(x)`.
                _ => break (self.spaced(&elements), None),
            };
            links.push(elements[1..].to_vec());
            if matches!(receiver.kind(), METHOD_CALL | FIELD_ACCESS) {
                current = receiver;
            } else {
                break (self.node(&receiver), Some(receiver));
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
                    Some(class) => class + 1,
                    None if base_name.chars().count() <= 4 => 1,
                    None => 0,
                }
            }
            Some(THIS_EXPR | SUPER_EXPR) => 1,
            _ => 0,
        }
        .min(links.len());

        let mut head = vec![base];
        head.extend(links[..attached].iter().map(|link| self.spaced(link)));
        let rest = &links[attached..];
        let calls = links.iter().filter(|link| is_call(link)).count();
        let simple_base = matches!(base_kind, Some(NAME_EXPR | THIS_EXPR | SUPER_EXPR));
        let lone_call = simple_base && calls == 1 && links.last().is_some_and(is_call);
        if rest.is_empty() || (calls == 0 && rest.len() < 2) {
            head.extend(rest.iter().map(|link| self.spaced(link)));
            return concat(head);
        }

        let tag = self.tag();
        if lone_call {
            let (last, fields) = rest.split_last().expect("rest is not empty");
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

        let mut tail = Vec::new();
        for link in rest {
            tail.push(Doc::SoftLine);
            tail.push(self.spaced(link));
        }
        tagged_group(
            tag,
            concat(vec![
                indent_if(tag, self.continuation_indent(), concat(head)),
                indent(self.continuation_indent(), concat(tail)),
            ]),
        )
    }

    /// A parenthesised, comma-separated list. If it does not fit after the
    /// `(`, it moves to the next line; if it still does not fit, every item
    /// gets its own line.
    fn list(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let (open, rest) = elements.split_first()?;
        let (close, items) = rest.split_last()?;
        if items.is_empty() {
            return Some(concat(vec![self.element(open), self.element(close)]));
        }
        // google-java-format 2.0.0: a block lambda that is the only argument
        // stays on the line of the call, `submit(() -> {`.
        if let [NodeOrToken::Node(only)] = items {
            let block_lambda = only.kind() == LAMBDA_EXPR
                && only
                    .children()
                    .last()
                    .is_some_and(|body| body.kind() == BLOCK);
            if node.kind() == ARG_LIST && block_lambda {
                return Some(concat(vec![
                    self.element(open),
                    self.node(only),
                    self.element(close),
                ]));
            }
        }
        Some(concat(vec![
            self.element(open),
            group(indent(
                self.continuation_indent(),
                concat(vec![Doc::SoftLine, group(self.comma_separated(items))]),
            )),
            self.element(close),
        ]))
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
    fn array_init(&self, node: &SyntaxNode) -> Option<Doc> {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let (open, rest) = elements.split_first()?;
        let (close, items) = rest.split_last()?;
        let close = close.as_token()?;
        if items.is_empty() && !self.has_dangling_comments(close) {
            return Some(concat(vec![self.element(open), self.token(close)]));
        }
        let mut inner = vec![Doc::SoftLine, group(self.comma_separated(items))];
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
