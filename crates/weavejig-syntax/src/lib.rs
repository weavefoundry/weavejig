//! Lossless Java syntax trees.
//!
//! [`parse`] turns source text into a concrete syntax tree that keeps every
//! byte of the input, including whitespace and comments, so the formatter and
//! the linter's fixes can work from exact source positions.
//!
//! The parser is hand-written and error-tolerant: it always produces a tree,
//! and reports what it could not understand as [`SyntaxError`]s.

mod grammar;
mod lexer;
mod line_index;
mod parser;
mod syntax_kind;

pub use crate::lexer::{Token, lex};
pub use crate::line_index::{LineCol, LineIndex};
pub use crate::syntax_kind::SyntaxKind;
pub use rowan::{Direction, NodeOrToken, TextRange, TextSize, TokenAtOffset, WalkEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum JavaLanguage {}

impl rowan::Language for JavaLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        SyntaxKind::from_raw(raw.0)
    }

    fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<JavaLanguage>;
pub type SyntaxToken = rowan::SyntaxToken<JavaLanguage>;
pub type SyntaxElement = rowan::SyntaxElement<JavaLanguage>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub message: String,
    pub range: TextRange,
}

/// The result of parsing one source file.
#[derive(Debug, Clone)]
pub struct Parse {
    green: rowan::GreenNode,
    errors: Vec<SyntaxError>,
}

impl Parse {
    /// The root `COMPILATION_UNIT` node.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    /// Lexical and syntax errors, in source order.
    pub fn errors(&self) -> &[SyntaxError] {
        &self.errors
    }

    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// An indented dump of the tree, without whitespace tokens. For tests
    /// and debugging.
    pub fn debug_tree(&self) -> String {
        let mut out = String::new();
        let mut depth = 0;
        for event in self.syntax().preorder_with_tokens() {
            match event {
                WalkEvent::Enter(element) => {
                    if element.kind() != SyntaxKind::WHITESPACE {
                        out.push_str(&"  ".repeat(depth));
                        match &element {
                            NodeOrToken::Node(node) => {
                                out.push_str(&format!("{:?}\n", node.kind()))
                            }
                            NodeOrToken::Token(token) => {
                                out.push_str(&format!("{:?} {:?}\n", token.kind(), token.text()));
                            }
                        }
                    }
                    depth += 1;
                }
                WalkEvent::Leave(_) => depth -= 1,
            }
        }
        out
    }
}

/// Parses a Java compilation unit.
pub fn parse(text: &str) -> Parse {
    let (tokens, lex_errors) = lex(text);
    let (green, mut errors) = parser::parse_tokens(text, &tokens);
    errors.extend(lex_errors);
    errors.sort_by_key(|e| (e.range.start(), e.range.end()));
    Parse { green, errors }
}

/// The first token of `node` that is not whitespace or a comment.
pub fn first_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    let mut token = node.first_token()?;
    let end = node.text_range().end();
    while token.kind().is_trivia() {
        token = token
            .next_token()
            .filter(|t| t.text_range().start() < end)?;
    }
    Some(token)
}

/// The last token of `node` that is not whitespace or a comment.
pub fn last_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    let mut token = node.last_token()?;
    let start = node.text_range().start();
    while token.kind().is_trivia() {
        token = token
            .prev_token()
            .filter(|t| t.text_range().end() > start)?;
    }
    Some(token)
}

/// The child tokens and nodes of `node`, without whitespace and comments.
pub fn significant_children(node: &SyntaxNode) -> impl Iterator<Item = SyntaxElement> {
    node.children_with_tokens()
        .filter(|el| !el.kind().is_trivia())
}

/// The first direct child token of `node` with the given kind.
pub fn child_token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind() == kind)
}

/// The first direct child node of `node` with the given kind.
pub fn child_node(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|n| n.kind() == kind)
}

/// The next token after `token` that is not whitespace or a comment.
pub fn next_significant_token(token: &SyntaxToken) -> Option<SyntaxToken> {
    let mut next = token.next_token()?;
    while next.kind().is_trivia() {
        next = next.next_token()?;
    }
    Some(next)
}

/// The previous token before `token` that is not whitespace or a comment.
pub fn prev_significant_token(token: &SyntaxToken) -> Option<SyntaxToken> {
    let mut prev = token.prev_token()?;
    while prev.kind().is_trivia() {
        prev = prev.prev_token()?;
    }
    Some(prev)
}

/// Modifier keywords in the order the Java Language Specification suggests
/// (§8.1.1, §8.3.1, §8.4.3), which is also the order google-java-format and
/// Checkstyle use.
pub const MODIFIER_ORDER: [&str; 14] = [
    "public",
    "protected",
    "private",
    "abstract",
    "default",
    "static",
    "sealed",
    "non-sealed",
    "final",
    "transient",
    "volatile",
    "synchronized",
    "native",
    "strictfp",
];

/// Where `keyword` comes in [`MODIFIER_ORDER`]; anything else sorts last.
pub fn modifier_rank(keyword: &str) -> usize {
    MODIFIER_ORDER
        .iter()
        .position(|k| *k == keyword)
        .unwrap_or(MODIFIER_ORDER.len())
}
