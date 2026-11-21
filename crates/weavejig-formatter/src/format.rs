//! Formatting rules: syntax tree in, layout document out.
//!
//! [`Formatter::node`] dispatches on the node kind. Kinds without a rule of
//! their own fall back to [`Formatter::generic`], which lays the node's
//! children out on one line using the spacing table. A rule exists wherever
//! a construct needs line breaks or indentation.

use std::cell::Cell;

use weavejig_syntax::{
    NodeOrToken, SyntaxElement, SyntaxKind, SyntaxKind::*, SyntaxNode, SyntaxToken, TextSize,
    first_token, last_token, significant_children,
};

use crate::FormatOptions;
use crate::comments::{Comment, Comments, TokenTrivia};
use crate::doc::{Doc, concat, group, indent, space, text};
use crate::spacing::space_between;

pub(crate) struct Formatter<'a> {
    comments: Comments,
    options: &'a FormatOptions,
    /// A token whose leading comments have already been printed elsewhere.
    skip_leading: Cell<Option<TextSize>>,
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
        Formatter {
            comments: Comments::collect(root),
            options,
            skip_leading: Cell::new(None),
        }
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
                let mut is_static = false;
                let mut name = String::new();
                for element in significant_children(import) {
                    match element.kind() {
                        IMPORT_KW | SEMI => {}
                        STATIC_KW => is_static = true,
                        _ => name.push_str(&element.to_string()),
                    }
                }
                (!is_static, name.split_whitespace().collect(), *import)
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

    /// Annotations on a declaration go on their own lines above it; on a
    /// parameter or local variable they stay inline. Fields are in between:
    /// their annotations stay inline unless one takes arguments.
    fn modifiers(&self, node: &SyntaxNode) -> Doc {
        let elements: Vec<SyntaxElement> = significant_children(node).collect();
        let leading = elements
            .iter()
            .take_while(|el| el.kind() == ANNOTATION)
            .count();
        let has_arguments = |element: &SyntaxElement| {
            element
                .as_node()
                .is_some_and(|a| a.children().any(|c| c.kind() == ANNOTATION_ARGS))
        };
        let vertical = match node.parent().map(|parent| parent.kind()) {
            Some(
                CLASS_DECL | INTERFACE_DECL | ENUM_DECL | RECORD_DECL | ANNOTATION_TYPE_DECL
                | METHOD_DECL | CONSTRUCTOR_DECL | ENUM_CONSTANT | PACKAGE_DECL | MODULE_DECL,
            ) => true,
            Some(FIELD_DECL) => elements[..leading].iter().any(has_arguments),
            _ => false,
        };
        if !vertical {
            return self.spaced(&elements);
        }
        let mut docs = Vec::new();
        for annotation in &elements[..leading] {
            docs.push(self.element(annotation));
            docs.push(Doc::HardLine);
        }
        docs.push(self.spaced(&elements[leading..]));
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

        let mut body = Vec::new();
        let mut previous: Option<SyntaxKind> = None;
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
            let kind = member.kind();
            // Fields, and enum constants, may be packed together or spaced
            // out as the author chose; everything else gets a blank line.
            let keeps_spacing = match previous {
                None => true,
                Some(previous) => {
                    previous == kind && matches!(kind, FIELD_DECL | ENUM_CONSTANT | EMPTY_DECL)
                }
            };
            body.push(if keeps_spacing {
                self.separator(member)
            } else {
                Doc::BlankLine
            });
            body.push(self.with_comments(&NodeOrToken::Node(member.clone())));
            previous = Some(kind);
        }
        self.dangling_comments(close, &mut body);
        Some(concat(vec![
            self.token(open),
            indent(self.block_indent(), concat(body)),
            Doc::HardLine,
            self.token_without_leading(close),
        ]))
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
            return Some(concat(vec![self.token(open), self.token(close)]));
        }
        let mut body = Vec::new();
        for (i, statement) in statements.iter().enumerate() {
            match statement.as_node() {
                Some(node) if i > 0 => body.push(self.separator(node)),
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
    /// The chain breaks before every `.` or before none, except that a
    /// leading run of plain names and the first call on them stay together:
    /// `System.out.println(...)` and `ImmutableList.builder()` never split.
    fn chain(&self, node: &SyntaxNode) -> Doc {
        // Each link is `.name` or `.name(args)`, innermost receiver last.
        let mut links: Vec<Vec<SyntaxElement>> = Vec::new();
        let mut current = node.clone();
        let (base, simple_base) = loop {
            let elements: Vec<SyntaxElement> = significant_children(&current).collect();
            let receiver = match (elements.first(), elements.get(1)) {
                (Some(NodeOrToken::Node(receiver)), Some(dot)) if dot.kind() == DOT => {
                    receiver.clone()
                }
                // A call without a receiver, such as `f(x)`.
                _ => break (self.spaced(&elements), false),
            };
            links.push(elements[1..].to_vec());
            if matches!(receiver.kind(), METHOD_CALL | FIELD_ACCESS) {
                current = receiver;
            } else {
                let simple = matches!(receiver.kind(), NAME_EXPR | THIS_EXPR | SUPER_EXPR);
                break (self.node(&receiver), simple);
            }
        };
        links.reverse();

        let is_call = |link: &Vec<SyntaxElement>| link.iter().any(|el| el.kind() == ARG_LIST);
        let attached = if simple_base {
            let names = links.iter().take_while(|link| !is_call(link)).count();
            (names + 1).min(links.len())
        } else {
            0
        };
        let mut head = vec![base];
        head.extend(links[..attached].iter().map(|link| self.spaced(link)));
        if attached == links.len() {
            return concat(head);
        }
        let mut rest = Vec::new();
        for link in &links[attached..] {
            rest.push(Doc::SoftLine);
            rest.push(self.spaced(link));
        }
        head.push(indent(self.continuation_indent(), concat(rest)));
        group(concat(head))
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
