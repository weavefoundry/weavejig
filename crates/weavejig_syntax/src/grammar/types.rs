//! Types, type arguments and type parameters, plus the token-level lookahead
//! used to tell types from expressions.

use super::annotation;
use crate::SyntaxKind::*;
use crate::parser::{CompletedMarker, Parser};

pub(super) fn at_type_start(p: &Parser<'_>) -> bool {
    matches!(p.current(), IDENT | AT) || p.current().is_primitive_type()
}

/// A type, including array dimensions.
pub(super) fn type_(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    let mut ty = type_no_dims(p)?;
    while p.at(L_BRACK) && p.nth(1) == R_BRACK {
        let m = ty.precede(p);
        p.bump();
        p.bump();
        ty = m.complete(p, ARRAY_TYPE);
    }
    Some(ty)
}

/// A type or `void`.
pub(super) fn type_or_void(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    if p.at(VOID_KW) {
        let m = p.start();
        p.bump();
        return Some(m.complete(p, PRIMITIVE_TYPE));
    }
    type_(p)
}

/// A primitive or class type without array dimensions, as after `new`.
pub(super) fn type_no_dims(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    if !p.enter() {
        return None;
    }
    let m = p.start();
    let mut annotated = false;
    while p.at(AT) {
        annotation(p);
        annotated = true;
    }
    let result = if p.current().is_primitive_type() {
        p.bump();
        Some(m.complete(p, PRIMITIVE_TYPE))
    } else if p.at(IDENT) {
        loop {
            p.bump();
            if p.at(LT) {
                type_args(p);
            }
            if p.at(DOT) && matches!(p.nth(1), IDENT | AT) {
                p.bump();
                while p.at(AT) {
                    annotation(p);
                }
                if p.at(IDENT) {
                    continue;
                }
                p.error("expected an identifier");
            }
            break;
        }
        Some(m.complete(p, NAMED_TYPE))
    } else {
        p.error("expected a type");
        if annotated {
            m.complete(p, ERROR);
        } else {
            m.abandon(p);
        }
        None
    };
    p.leave();
    result
}

/// `<A, ? extends B>`. The caller must be at `<`.
pub(super) fn type_args(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump();
    if !p.at(GT) {
        loop {
            if p.at(QUESTION) {
                let wildcard = p.start();
                p.bump();
                if p.at(EXTENDS_KW) || p.at(SUPER_KW) {
                    p.bump();
                    type_(p);
                }
                wildcard.complete(p, WILDCARD_TYPE);
            } else {
                type_(p);
            }
            if !p.eat(COMMA) {
                break;
            }
        }
    }
    p.expect(GT);
    m.complete(p, TYPE_ARGS);
}

/// `<T, U extends Comparable<U> & Serializable>`. The caller must be at `<`.
pub(super) fn type_params(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump();
    loop {
        let param = p.start();
        while p.at(AT) {
            annotation(p);
        }
        p.expect(IDENT);
        if p.at(EXTENDS_KW) {
            let bound = p.start();
            p.bump();
            loop {
                type_(p);
                if !p.eat(AMP) {
                    break;
                }
            }
            bound.complete(p, TYPE_BOUND);
        }
        param.complete(p, TYPE_PARAM);
        if !p.eat(COMMA) {
            break;
        }
    }
    p.expect(GT);
    m.complete(p, TYPE_PARAMS);
}

// --- Lookahead --------------------------------------------------------------
//
// These scan tokens without building anything. They answer "could a type
// start here, and where would it end?", which is what distinguishes
// declarations, casts and patterns from expressions.

/// If a type starts at token `i`, returns the index just past it.
pub(super) fn la_type(p: &Parser<'_>, i: usize) -> Option<usize> {
    let mut i = la_annotations(p, i);
    if p.kind_at(i).is_primitive_type() {
        i += 1;
    } else if p.kind_at(i) == IDENT {
        loop {
            i += 1;
            if p.kind_at(i) == LT {
                i = la_type_args(p, i)?;
            }
            if p.kind_at(i) == DOT {
                let next = la_annotations(p, i + 1);
                if p.kind_at(next) == IDENT {
                    i = next;
                    continue;
                }
            }
            break;
        }
    } else {
        return None;
    }
    loop {
        let next = la_annotations(p, i);
        if p.kind_at(next) == L_BRACK && p.kind_at(next + 1) == R_BRACK {
            i = next + 2;
        } else {
            return Some(i);
        }
    }
}

/// Skips any annotations starting at token `i`.
pub(super) fn la_annotations(p: &Parser<'_>, mut i: usize) -> usize {
    while p.kind_at(i) == AT && p.kind_at(i + 1) == IDENT {
        i += 2;
        while p.kind_at(i) == DOT && p.kind_at(i + 1) == IDENT {
            i += 2;
        }
        if p.kind_at(i) == L_PAREN {
            match la_matching_paren(p, i) {
                Some(close) => i = close + 1,
                None => return i,
            }
        }
    }
    i
}

/// Index of the `)` matching the `(` at token `i`.
pub(super) fn la_matching_paren(p: &Parser<'_>, mut i: usize) -> Option<usize> {
    let mut depth = 0u32;
    loop {
        match p.kind_at(i) {
            L_PAREN => depth += 1,
            R_PAREN => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            EOF => return None,
            _ => {}
        }
        i += 1;
    }
}

/// If balanced type arguments start at the `<` at token `i`, returns the
/// index just past the closing `>`.
fn la_type_args(p: &Parser<'_>, mut i: usize) -> Option<usize> {
    let mut depth = 0u32;
    loop {
        match p.kind_at(i) {
            LT => depth += 1,
            GT => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            IDENT | DOT | COMMA | QUESTION | EXTENDS_KW | SUPER_KW | AMP | L_BRACK | R_BRACK
            | AT => {}
            kind if kind.is_primitive_type() => {}
            _ => return None,
        }
        i += 1;
    }
}
