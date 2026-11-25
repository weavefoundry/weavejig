//! Tokens that do nothing, which google-java-format removes:
//!
//! - the `;` of an empty statement in a block or under a `case`, and of an
//!   empty declaration in a class body or between top-level declarations;
//! - the `()` of an annotation that is given no values: `@Override()`;
//! - the parentheses around a single lambda parameter without a type:
//!   `(x) -> x`.
//!
//! Comments next to such a token stay.

use weavejig_syntax::{SyntaxKind::*, SyntaxNode, SyntaxToken, TextRange, significant_children};

/// The tokens of `root` that do nothing, in source order.
pub(crate) fn tokens(root: &SyntaxNode) -> Vec<TextRange> {
    let mut ranges = Vec::new();
    for node in root.descendants() {
        let parent = node.parent().map(|parent| parent.kind());
        match node.kind() {
            // An empty statement that is the body of an `if` or a loop, or
            // that a label names, is the statement there has to be.
            EMPTY_STMT if matches!(parent, Some(BLOCK | SWITCH_GROUP)) => {
                ranges.extend(only_token(&node, SEMI));
            }
            EMPTY_DECL if matches!(parent, Some(CLASS_BODY | COMPILATION_UNIT)) => {
                ranges.extend(only_token(&node, SEMI));
            }
            ANNOTATION_ARGS => ranges.extend(brackets(&node, 0)),
            LAMBDA_PARAMS if is_bare_name(&node) => ranges.extend(brackets(&node, 1)),
            _ => {}
        }
    }
    ranges.sort_by_key(|range| range.start());
    ranges
}

/// The node's token of this kind, if it is all the node has.
fn only_token(node: &SyntaxNode, kind: weavejig_syntax::SyntaxKind) -> Option<TextRange> {
    let mut tokens = tokens_of(node);
    let only = tokens.next().filter(|token| token.kind() == kind)?;
    tokens.next().is_none().then(|| only.text_range())
}

/// Every token of a node other than whitespace: comments count.
fn tokens_of(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() != WHITESPACE)
}

/// The `(` and `)` of a node that has `between` tokens between them, and
/// no comment.
fn brackets(node: &SyntaxNode, between: usize) -> Vec<TextRange> {
    let tokens: Vec<SyntaxToken> = tokens_of(node).collect();
    match (tokens.first(), tokens.last()) {
        (Some(open), Some(close))
            if tokens.len() == between + 2 && open.kind() == L_PAREN && close.kind() == R_PAREN =>
        {
            vec![open.text_range(), close.text_range()]
        }
        _ => Vec::new(),
    }
}

/// Whether lambda parameters are one name: no type, no modifiers.
fn is_bare_name(params: &SyntaxNode) -> bool {
    let children: Vec<_> = significant_children(params).collect();
    let [_, parameter, _] = children.as_slice() else {
        return false;
    };
    match parameter {
        weavejig_syntax::NodeOrToken::Token(token) => token.kind() == IDENT,
        weavejig_syntax::NodeOrToken::Node(node) => {
            let mut tokens = tokens_of(node);
            tokens.next().is_some_and(|token| token.kind() == IDENT) && tokens.next().is_none()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weavejig_syntax::parse;

    fn removed(text: &str) -> Vec<String> {
        tokens(&parse(text).syntax())
            .into_iter()
            .map(|range| format!("{}@{}", &text[range], u32::from(range.start())))
            .collect()
    }

    #[test]
    fn empty_statements_and_declarations() {
        assert_eq!(removed("class A { ; int x;; }"), [";@10", ";@18"]);
        assert_eq!(
            removed("class A { void m() { ; f();; } }"),
            [";@21", ";@27"]
        );
        // The body of a loop or an `if`, and a labelled statement, stay.
        assert!(removed("class A { void m() { for (;;) ; if (a) ; else ; l: ; } }").is_empty());
        // So does the `;` that ends an enum's constants.
        assert_eq!(removed("enum E { A,; }"), Vec::<String>::new());
        assert_eq!(removed("enum E { A;; int x; }"), [";@11"]);
    }

    #[test]
    fn annotation_and_lambda_parentheses() {
        assert_eq!(removed("@A() class B {}"), ["(@2", ")@3"]);
        assert!(removed("@A(1) class B {}").is_empty());
        assert!(removed("@A(/* c */) class B {}").is_empty());
        let lambdas = "class A { Object o = f((x) -> x, (int y) -> y, (a, b) -> a, () -> 1); }";
        assert_eq!(removed(lambdas), ["(@23", ")@25"]);
    }
}
