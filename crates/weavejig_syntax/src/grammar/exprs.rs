//! Expressions and patterns.

use super::types::{self, la_annotations, la_matching_paren, la_type};
use super::{annotation, class_body, modifiers, param, stmts};
use crate::SyntaxKind::*;
use crate::parser::{CompletedMarker, Marker, Parser};

pub(super) fn expr(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    if !p.enter() {
        return None;
    }
    let result = assignment(p);
    p.leave();
    result
}

fn assignment(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    let lhs = conditional(p)?;
    if p.current().is_assignment_op() {
        let m = lhs.precede(p);
        p.bump();
        expr(p);
        return Some(m.complete(p, ASSIGN_EXPR));
    }
    Some(lhs)
}

/// A conditional expression: everything except assignment. Annotation element
/// values and `case` constants use this level.
pub(super) fn conditional(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    let condition = binary(p, 0)?;
    if !p.at(QUESTION) {
        return Some(condition);
    }
    let m = condition.precede(p);
    p.bump();
    expr(p);
    p.expect(COLON);
    if p.enter() {
        conditional(p);
        p.leave();
    }
    Some(m.complete(p, CONDITIONAL_EXPR))
}

/// Binding power and token count of the binary operator at the current
/// position. Shifts are two or three adjacent `>` tokens.
fn binary_op(p: &Parser<'_>) -> Option<(u8, usize)> {
    let op = match p.current() {
        PIPE_PIPE => (1, 1),
        AMP_AMP => (2, 1),
        PIPE => (3, 1),
        CARET => (4, 1),
        AMP => (5, 1),
        EQ_EQ | BANG_EQ => (6, 1),
        GT if p.nth(1) == GT && p.joint_at(p.pos()) => {
            if p.nth(2) == GT && p.joint_at(p.pos() + 1) {
                (8, 3)
            } else {
                (8, 2)
            }
        }
        LT | GT | LT_EQ | GT_EQ | INSTANCEOF_KW => (7, 1),
        LT_LT => (8, 1),
        PLUS | MINUS => (9, 1),
        STAR | SLASH | PERCENT => (10, 1),
        _ => return None,
    };
    Some(op)
}

fn binary(p: &mut Parser<'_>, min_power: u8) -> Option<CompletedMarker> {
    let mut lhs = unary(p)?;
    while let Some((power, tokens)) = binary_op(p) {
        if power <= min_power {
            break;
        }
        let m = lhs.precede(p);
        if p.at(INSTANCEOF_KW) {
            p.bump();
            let at_pattern = matches!(p.current(), FINAL_KW | AT)
                || la_type(p, p.pos()).is_some_and(|end| matches!(p.kind_at(end), IDENT | L_PAREN));
            if at_pattern {
                pattern(p);
            } else {
                types::type_(p);
            }
            lhs = m.complete(p, INSTANCEOF_EXPR);
        } else {
            for _ in 0..tokens {
                p.bump();
            }
            binary(p, power);
            lhs = m.complete(p, BINARY_EXPR);
        }
    }
    Some(lhs)
}

/// A type pattern (`String s`) or record pattern (`Point(int x, var y)`).
pub(super) fn pattern(p: &mut Parser<'_>) {
    if !p.enter() {
        return;
    }
    let m = p.start();
    modifiers(p, false);
    types::type_(p);
    if p.eat(L_PAREN) {
        if !p.at(R_PAREN) {
            loop {
                pattern(p);
                if !p.eat(COMMA) {
                    break;
                }
            }
        }
        p.expect(R_PAREN);
        m.complete(p, RECORD_PATTERN);
    } else {
        p.expect(IDENT);
        m.complete(p, TYPE_PATTERN);
    }
    p.leave();
}

fn unary(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    if !p.enter() {
        return None;
    }
    let result = match p.current() {
        PLUS | MINUS | PLUS_PLUS | MINUS_MINUS | BANG | TILDE => {
            let m = p.start();
            p.bump();
            unary(p);
            Some(m.complete(p, PREFIX_EXPR))
        }
        L_PAREN if !p.no_lambda && at_paren_lambda(p) => Some(lambda(p)),
        L_PAREN if at_cast(p) => {
            let m = p.start();
            p.bump();
            if let Some(first) = types::type_(p)
                && p.at(AMP)
            {
                let intersection = first.precede(p);
                while p.eat(AMP) {
                    types::type_(p);
                }
                intersection.complete(p, INTERSECTION_TYPE);
            }
            p.expect(R_PAREN);
            unary(p);
            Some(m.complete(p, CAST_EXPR))
        }
        _ => postfix(p),
    };
    p.leave();
    result
}

/// At `(`: is this the parameter list of a lambda?
fn at_paren_lambda(p: &Parser<'_>) -> bool {
    la_matching_paren(p, p.pos()).is_some_and(|close| p.kind_at(close + 1) == ARROW)
}

/// At `(`: is this a cast? True when the parentheses hold a type and what
/// follows can only be the operand of a cast.
fn at_cast(p: &Parser<'_>) -> bool {
    let start = p.pos() + 1;
    let Some(mut end) = la_type(p, start) else {
        return false;
    };
    while p.kind_at(end) == AMP {
        match la_type(p, end + 1) {
            Some(next) => end = next,
            None => return false,
        }
    }
    if p.kind_at(end) != R_PAREN {
        return false;
    }
    // `(int)` cannot be a parenthesised expression, so whatever follows is
    // the operand, including `+x` and `-x`.
    let base = la_annotations(p, start);
    if p.kind_at(base).is_primitive_type() && end == base + 1 {
        return true;
    }
    // After `(Name)`, `+` and `-` are binary operators.
    match p.kind_at(end + 1) {
        IDENT | L_PAREN | BANG | TILDE | THIS_KW | SUPER_KW | NEW_KW | SWITCH_KW | VOID_KW => true,
        kind => kind.is_literal() || kind.is_primitive_type(),
    }
}

fn lambda(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    if p.at(IDENT) {
        let param = p.start();
        p.bump();
        param.complete(p, PARAM);
    } else {
        let params = p.start();
        p.bump(); // `(`
        if p.at(IDENT) && matches!(p.nth(1), COMMA | R_PAREN) {
            loop {
                let param = p.start();
                p.expect(IDENT);
                param.complete(p, PARAM);
                if !p.eat(COMMA) {
                    break;
                }
            }
        } else if !p.at(R_PAREN) {
            loop {
                param(p);
                if !p.eat(COMMA) {
                    break;
                }
            }
        }
        p.expect(R_PAREN);
        params.complete(p, LAMBDA_PARAMS);
    }
    p.expect(ARROW);
    if p.at(L_BRACE) {
        stmts::block(p);
    } else {
        let saved = std::mem::replace(&mut p.no_lambda, false);
        expr(p);
        p.no_lambda = saved;
    }
    m.complete(p, LAMBDA_EXPR)
}

fn postfix(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    let mut lhs = primary(p)?;
    loop {
        lhs = match p.current() {
            DOT => {
                let m = lhs.precede(p);
                p.bump();
                match p.current() {
                    IDENT => {
                        p.bump();
                        if p.at(L_PAREN) {
                            arg_list(p);
                            m.complete(p, METHOD_CALL)
                        } else {
                            m.complete(p, FIELD_ACCESS)
                        }
                    }
                    LT => {
                        types::type_args(p);
                        if !p.eat(SUPER_KW) {
                            p.expect(IDENT);
                        }
                        if p.at(L_PAREN) {
                            arg_list(p);
                        } else {
                            p.error("expected `(`");
                        }
                        m.complete(p, METHOD_CALL)
                    }
                    NEW_KW => new_expr(p, m),
                    THIS_KW => {
                        p.bump();
                        m.complete(p, THIS_EXPR)
                    }
                    SUPER_KW => {
                        p.bump();
                        m.complete(p, SUPER_EXPR)
                    }
                    CLASS_KW => {
                        p.bump();
                        m.complete(p, CLASS_LITERAL)
                    }
                    _ => {
                        p.error("expected an identifier");
                        m.complete(p, FIELD_ACCESS)
                    }
                }
            }
            L_BRACK => {
                let m = lhs.precede(p);
                p.bump();
                expr(p);
                p.expect(R_BRACK);
                m.complete(p, ARRAY_ACCESS)
            }
            COLON_COLON => {
                let m = lhs.precede(p);
                p.bump();
                if p.at(LT) {
                    types::type_args(p);
                }
                if !p.eat(NEW_KW) {
                    p.expect(IDENT);
                }
                m.complete(p, METHOD_REF)
            }
            PLUS_PLUS | MINUS_MINUS => {
                let m = lhs.precede(p);
                p.bump();
                m.complete(p, POSTFIX_EXPR)
            }
            // Explicit constructor invocation: `this(...)`, `outer.super(...)`.
            L_PAREN if matches!(lhs.kind(), THIS_EXPR | SUPER_EXPR) => {
                let m = lhs.precede(p);
                arg_list(p);
                m.complete(p, METHOD_CALL)
            }
            _ => break,
        };
    }
    Some(lhs)
}

fn primary(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    let kind = p.current();
    if kind.is_literal() {
        let m = p.start();
        p.bump();
        return Some(m.complete(p, LITERAL));
    }
    if kind.is_primitive_type() || kind == VOID_KW {
        // `int.class`, `int[]::new`
        return types::type_or_void(p);
    }
    match kind {
        IDENT => {
            if !p.no_lambda && p.nth(1) == ARROW {
                return Some(lambda(p));
            }
            if matches!(p.nth(1), LT | L_BRACK | DOT) && at_type_primary(p) {
                return types::type_(p);
            }
            let m = p.start();
            p.bump();
            if p.at(L_PAREN) {
                arg_list(p);
                Some(m.complete(p, METHOD_CALL))
            } else {
                Some(m.complete(p, NAME_EXPR))
            }
        }
        THIS_KW => {
            let m = p.start();
            p.bump();
            Some(m.complete(p, THIS_EXPR))
        }
        SUPER_KW => {
            let m = p.start();
            p.bump();
            Some(m.complete(p, SUPER_EXPR))
        }
        L_PAREN => {
            let m = p.start();
            p.bump();
            let saved = std::mem::replace(&mut p.no_lambda, false);
            expr(p);
            p.no_lambda = saved;
            p.expect(R_PAREN);
            Some(m.complete(p, PAREN_EXPR))
        }
        NEW_KW => {
            let m = p.start();
            Some(new_expr(p, m))
        }
        SWITCH_KW => {
            let m = p.start();
            stmts::switch_rest(p);
            Some(m.complete(p, SWITCH_EXPR))
        }
        _ => {
            p.error("expected an expression");
            None
        }
    }
}

/// At an identifier: does a generic or array type start here, as the
/// qualifier of a method reference (`List<String>::new`, `int[]::clone`) or
/// class literal (`String[].class`)?
fn at_type_primary(p: &Parser<'_>) -> bool {
    let start = p.pos();
    let Some(end) = la_type(p, start) else {
        return false;
    };
    (start..end).any(|i| matches!(p.kind_at(i), LT | L_BRACK))
        && (p.kind_at(end) == COLON_COLON
            || (p.kind_at(end) == DOT && p.kind_at(end + 1) == CLASS_KW))
}

/// An object or array creation, starting at `new`. `m` already holds the
/// qualifier of an inner class creation (`outer.new Inner()`), if any.
fn new_expr(p: &mut Parser<'_>, m: Marker) -> CompletedMarker {
    p.bump(); // `new`
    if p.at(LT) {
        types::type_args(p);
    }
    types::type_no_dims(p);
    if p.at(L_BRACK) {
        while p.at(L_BRACK) {
            if p.nth(1) == R_BRACK {
                p.bump();
                p.bump();
            } else {
                let dim = p.start();
                p.bump();
                expr(p);
                p.expect(R_BRACK);
                dim.complete(p, DIM_EXPR);
            }
        }
        if p.at(L_BRACE) {
            array_init(p);
        }
        return m.complete(p, ARRAY_CREATION_EXPR);
    }
    if p.at(L_PAREN) {
        arg_list(p);
    } else {
        p.error("expected `(` or `[`");
    }
    if p.at(L_BRACE) {
        class_body(p);
    }
    m.complete(p, NEW_EXPR)
}

/// `(a, b)`. The caller must be at `(`.
pub(super) fn arg_list(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump();
    let saved = std::mem::replace(&mut p.no_lambda, false);
    if !p.at(R_PAREN) {
        loop {
            expr(p);
            if !p.eat(COMMA) {
                break;
            }
        }
    }
    p.no_lambda = saved;
    p.expect(R_PAREN);
    m.complete(p, ARG_LIST);
}

/// `{a, b}` as an array initializer or annotation element value. The caller
/// must be at `{`.
pub(super) fn array_init(p: &mut Parser<'_>) {
    if !p.enter() {
        return;
    }
    let m = p.start();
    p.bump();
    while !p.at(R_BRACE) && !p.at_end() {
        match p.current() {
            L_BRACE => array_init(p),
            AT => annotation(p),
            _ => {
                expr(p);
            }
        }
        if !p.eat(COMMA) {
            break;
        }
    }
    p.expect(R_BRACE);
    m.complete(p, ARRAY_INIT);
    p.leave();
}
