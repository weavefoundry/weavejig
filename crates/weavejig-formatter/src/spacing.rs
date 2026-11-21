//! Whether two adjacent tokens are separated by a space.
//!
//! This is the default horizontal layout for every construct. Formatting
//! rules that need line breaks build on top of it.

use weavejig_syntax::{SyntaxKind, SyntaxKind::*, SyntaxToken};

fn parent_kind(token: &SyntaxToken) -> SyntaxKind {
    token.parent().map_or(ERROR, |parent| parent.kind())
}

fn grandparent_kind(token: &SyntaxToken) -> SyntaxKind {
    token
        .parent()
        .and_then(|parent| parent.parent())
        .map_or(ERROR, |node| node.kind())
}

fn is_sign(kind: SyntaxKind) -> bool {
    matches!(kind, PLUS | MINUS | PLUS_PLUS | MINUS_MINUS)
}

/// Whether a space goes between `left` and `right`, two tokens that are
/// adjacent once whitespace and comments are ignored.
pub(crate) fn space_between(left: &SyntaxToken, right: &SyntaxToken) -> bool {
    let (lk, rk) = (left.kind(), right.kind());
    let (lp, rp) = (parent_kind(left), parent_kind(right));

    // `non-sealed` is three tokens.
    if lp == MODIFIERS && rp == MODIFIERS && (lk == MINUS || rk == MINUS) {
        return false;
    }
    // A prefix operator binds to its operand, but `- -x` must not become `--x`.
    if lp == PREFIX_EXPR && (is_sign(lk) || matches!(lk, BANG | TILDE)) {
        return is_sign(lk) && is_sign(rk) && left.text()[..1] == right.text()[..1];
    }

    match rk {
        // `int @Nullable []` and `String @NonNull ... args`: an annotation on
        // array brackets or on a varargs ellipsis is set off from them.
        L_BRACK | ELLIPSIS if follows_annotation(right) => return true,
        // An empty `for (;;)` header is written `for (; ; )`.
        SEMI | R_PAREN if rp == FOR_STMT && lk == SEMI => return true,
        SEMI | COMMA | R_PAREN | R_BRACK | DOT | ELLIPSIS | COLON_COLON | L_BRACK => return false,
        PLUS_PLUS | MINUS_MINUS if rp == POSTFIX_EXPR => return false,
        COLON if matches!(rp, LABELED_STMT | SWITCH_GROUP) => return false,
        GT if matches!(rp, TYPE_ARGS | TYPE_PARAMS) => return false,
        LT if rp == TYPE_ARGS => return lk == NEW_KW,
        // `class Box<T>` but `public <T> T get()`: no space when the name the
        // parameters belong to comes right before them.
        LT if rp == TYPE_PARAMS => {
            return left.parent() != right.parent().and_then(|params| params.parent());
        }
        L_PAREN => return space_before_paren(left, lk, lp),
        _ => {}
    }

    match lk {
        L_PAREN | L_BRACK | DOT | AT | COLON_COLON => false,
        LT if matches!(lp, TYPE_ARGS | TYPE_PARAMS) => false,
        // `List<String> names` but `Collections.<String>emptyList()`.
        GT if lp == TYPE_ARGS => !matches!(grandparent_kind(left), METHOD_CALL | METHOD_REF),
        _ => true,
    }
}

/// Whether the element before `token` in its parent, ignoring whitespace and
/// comments, is an annotation.
fn follows_annotation(token: &SyntaxToken) -> bool {
    std::iter::successors(token.prev_sibling_or_token(), |element| {
        element.prev_sibling_or_token()
    })
    .find(|element| !element.kind().is_trivia())
    .is_some_and(|element| element.kind() == ANNOTATION)
}

fn space_before_paren(left: &SyntaxToken, lk: SyntaxKind, lp: SyntaxKind) -> bool {
    match lk {
        // `yield (x);` and `when (x)` use contextual keywords.
        IDENT => matches!(lp, YIELD_STMT | GUARD) && matches!(left.text(), "yield" | "when"),
        THIS_KW | SUPER_KW | L_PAREN | L_BRACK | R_BRACK | DOT | AT | COLON_COLON => false,
        GT if matches!(lp, TYPE_ARGS | TYPE_PARAMS) => false,
        // `(int) (a + b)` but `f()()`.
        R_PAREN => lp == CAST_EXPR,
        // Keywords (`if (`, `catch (`, `return (`), operators and separators.
        _ => true,
    }
}
