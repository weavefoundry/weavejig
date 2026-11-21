//! Blocks and statements.

use super::types::la_type;
use super::{at_modifier, at_type_decl, declarators, exprs, modifiers, type_decl, types};
use crate::SyntaxKind::*;
use crate::parser::{Marker, Parser};

pub(super) fn block(p: &mut Parser<'_>) {
    if !p.at(L_BRACE) {
        p.error("expected `{`");
        return;
    }
    let m = p.start();
    p.bump();
    statements(p, false);
    p.expect(R_BRACE);
    m.complete(p, BLOCK);
}

/// Statements up to a closing `}` or, inside a switch, the next label.
fn statements(p: &mut Parser<'_>, in_switch: bool) {
    while !p.at(R_BRACE) && !p.at_end() {
        if in_switch && matches!(p.current(), CASE_KW | DEFAULT_KW) {
            break;
        }
        let before = p.pos();
        statement(p);
        if p.pos() == before {
            p.err_and_bump("expected a statement");
        }
    }
}

pub(super) fn statement(p: &mut Parser<'_>) {
    if !p.enter() {
        return;
    }
    statement_inner(p);
    p.leave();
}

fn statement_inner(p: &mut Parser<'_>) {
    match p.current() {
        L_BRACE => block(p),
        SEMI => {
            let m = p.start();
            p.bump();
            m.complete(p, EMPTY_STMT);
        }
        IF_KW => {
            let m = p.start();
            p.bump();
            condition(p);
            statement(p);
            if p.eat(ELSE_KW) {
                statement(p);
            }
            m.complete(p, IF_STMT);
        }
        WHILE_KW => {
            let m = p.start();
            p.bump();
            condition(p);
            statement(p);
            m.complete(p, WHILE_STMT);
        }
        DO_KW => {
            let m = p.start();
            p.bump();
            statement(p);
            p.expect(WHILE_KW);
            condition(p);
            p.expect(SEMI);
            m.complete(p, DO_STMT);
        }
        FOR_KW => for_stmt(p),
        TRY_KW => try_stmt(p),
        SWITCH_KW => {
            let m = p.start();
            switch_rest(p);
            m.complete(p, SWITCH_STMT);
        }
        SYNCHRONIZED_KW => {
            let m = p.start();
            p.bump();
            condition(p);
            block(p);
            m.complete(p, SYNCHRONIZED_STMT);
        }
        RETURN_KW => {
            let m = p.start();
            p.bump();
            if !p.at(SEMI) {
                exprs::expr(p);
            }
            p.expect(SEMI);
            m.complete(p, RETURN_STMT);
        }
        THROW_KW => {
            let m = p.start();
            p.bump();
            exprs::expr(p);
            p.expect(SEMI);
            m.complete(p, THROW_STMT);
        }
        BREAK_KW | CONTINUE_KW => {
            let kind = if p.at(BREAK_KW) {
                BREAK_STMT
            } else {
                CONTINUE_STMT
            };
            let m = p.start();
            p.bump();
            p.eat(IDENT);
            p.expect(SEMI);
            m.complete(p, kind);
        }
        ASSERT_KW => {
            let m = p.start();
            p.bump();
            exprs::expr(p);
            if p.eat(COLON) {
                exprs::expr(p);
            }
            p.expect(SEMI);
            m.complete(p, ASSERT_STMT);
        }
        IDENT if p.nth(1) == COLON => {
            let m = p.start();
            p.bump();
            p.bump();
            statement(p);
            m.complete(p, LABELED_STMT);
        }
        IDENT if at_yield(p) => {
            let m = p.start();
            p.bump();
            exprs::expr(p);
            p.expect(SEMI);
            m.complete(p, YIELD_STMT);
        }
        _ => declaration_or_expr_stmt(p),
    }
}

/// `( expr )` after `if`, `while`, `switch` and `synchronized`.
fn condition(p: &mut Parser<'_>) {
    p.expect(L_PAREN);
    exprs::expr(p);
    p.expect(R_PAREN);
}

/// `yield` is a contextual keyword: `yield x;` is a statement, but
/// `yield = 1;` and `yield.foo();` use a variable named `yield`.
fn at_yield(p: &Parser<'_>) -> bool {
    if !p.at_word("yield") {
        return false;
    }
    match p.nth(1) {
        DOT | L_BRACK | SEMI | COMMA | R_PAREN | COLON_COLON => false,
        PLUS_PLUS | MINUS_MINUS => p.nth(2) != SEMI,
        kind => !kind.is_assignment_op(),
    }
}

fn declaration_or_expr_stmt(p: &mut Parser<'_>) {
    if at_modifier(p, false) || at_type_decl(p) {
        let m = p.start();
        modifiers(p, false);
        if at_type_decl(p) {
            type_decl(p, m);
        } else {
            local_var_rest(p, m);
        }
        return;
    }
    if at_local_var_decl(p) {
        let m = p.start();
        local_var_rest(p, m);
        return;
    }
    let before = p.pos();
    let m = p.start();
    if exprs::expr(p).is_none() && p.pos() == before {
        m.abandon(p);
        return;
    }
    p.expect(SEMI);
    m.complete(p, EXPR_STMT);
}

fn local_var_rest(p: &mut Parser<'_>, m: Marker) {
    types::type_(p);
    declarators(p);
    p.expect(SEMI);
    m.complete(p, LOCAL_VAR_DECL);
}

/// Whether a local variable declaration (without modifiers) starts here: a
/// type followed by a name and then something only a declaration allows.
fn at_local_var_decl(p: &Parser<'_>) -> bool {
    match la_type(p, p.pos()) {
        Some(end) => {
            p.kind_at(end) == IDENT
                && matches!(p.kind_at(end + 1), EQ | SEMI | COMMA | L_BRACK | COLON)
        }
        None => false,
    }
}

fn for_stmt(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump();
    p.expect(L_PAREN);
    if at_modifier(p, false) || at_local_var_decl(p) {
        let decl = p.start();
        modifiers(p, false);
        types::type_(p);
        if p.at(IDENT) && is_foreach_variable(p) {
            let var = p.start();
            p.bump();
            super::dims(p);
            var.complete(p, VARIABLE_DECLARATOR);
            decl.complete(p, LOCAL_VAR_DECL);
            p.expect(COLON);
            exprs::expr(p);
            p.expect(R_PAREN);
            statement(p);
            m.complete(p, FOREACH_STMT);
            return;
        }
        declarators(p);
        decl.complete(p, LOCAL_VAR_DECL);
    } else if !p.at(SEMI) {
        expr_list(p);
    }
    p.expect(SEMI);
    if !p.at(SEMI) {
        exprs::expr(p);
    }
    p.expect(SEMI);
    if !p.at(R_PAREN) {
        expr_list(p);
    }
    p.expect(R_PAREN);
    statement(p);
    m.complete(p, FOR_STMT);
}

/// At the variable name of a `for` header: is it followed by `:`?
fn is_foreach_variable(p: &Parser<'_>) -> bool {
    let mut i = p.pos() + 1;
    while p.kind_at(i) == L_BRACK && p.kind_at(i + 1) == R_BRACK {
        i += 2;
    }
    p.kind_at(i) == COLON
}

fn expr_list(p: &mut Parser<'_>) {
    loop {
        exprs::expr(p);
        if !p.eat(COMMA) {
            break;
        }
    }
}

fn try_stmt(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump();
    let mut handled = p.at(L_PAREN);
    if p.at(L_PAREN) {
        let resources = p.start();
        p.bump();
        while !p.at(R_PAREN) && !p.at_end() {
            let resource = p.start();
            if at_modifier(p, false) || at_local_var_decl(p) {
                modifiers(p, false);
                types::type_(p);
                p.expect(IDENT);
                p.expect(EQ);
            }
            exprs::expr(p);
            resource.complete(p, RESOURCE);
            if !p.eat(SEMI) {
                break;
            }
        }
        p.expect(R_PAREN);
        resources.complete(p, RESOURCE_LIST);
    }
    block(p);
    while p.at(CATCH_KW) {
        handled = true;
        let clause = p.start();
        p.bump();
        p.expect(L_PAREN);
        let param = p.start();
        modifiers(p, false);
        if let Some(first) = types::type_(p)
            && p.at(PIPE)
        {
            let union = first.precede(p);
            while p.eat(PIPE) {
                types::type_(p);
            }
            union.complete(p, UNION_TYPE);
        }
        p.expect(IDENT);
        param.complete(p, PARAM);
        p.expect(R_PAREN);
        block(p);
        clause.complete(p, CATCH_CLAUSE);
    }
    if p.at(FINALLY_KW) {
        handled = true;
        let clause = p.start();
        p.bump();
        block(p);
        clause.complete(p, FINALLY_CLAUSE);
    }
    if !handled {
        p.error("`try` without `catch`, `finally` or resources");
    }
    m.complete(p, TRY_STMT);
}

/// The rest of a switch statement or expression, starting at `switch`.
pub(super) fn switch_rest(p: &mut Parser<'_>) {
    p.bump();
    condition(p);
    if !p.at(L_BRACE) {
        p.error("expected `{`");
        return;
    }
    let m = p.start();
    p.bump();
    while !p.at(R_BRACE) && !p.at_end() {
        if !matches!(p.current(), CASE_KW | DEFAULT_KW) {
            p.err_and_bump("expected `case`, `default` or `}`");
            continue;
        }
        let entry = p.start();
        case_label(p);
        if p.eat(ARROW) {
            match p.current() {
                L_BRACE => block(p),
                THROW_KW => statement(p),
                _ => {
                    let body = p.start();
                    exprs::expr(p);
                    p.expect(SEMI);
                    body.complete(p, EXPR_STMT);
                }
            }
            entry.complete(p, SWITCH_RULE);
        } else {
            p.expect(COLON);
            while matches!(p.current(), CASE_KW | DEFAULT_KW) {
                case_label(p);
                p.expect(COLON);
            }
            statements(p, true);
            entry.complete(p, SWITCH_GROUP);
        }
    }
    p.expect(R_BRACE);
    m.complete(p, SWITCH_BLOCK);
}

fn case_label(p: &mut Parser<'_>) {
    let m = p.start();
    if !p.eat(DEFAULT_KW) {
        p.bump(); // `case`
        let saved = std::mem::replace(&mut p.no_lambda, true);
        loop {
            if p.at(DEFAULT_KW) {
                p.bump(); // `case null, default`
            } else if at_pattern(p) {
                exprs::pattern(p);
            } else {
                exprs::conditional(p);
            }
            if !p.eat(COMMA) {
                break;
            }
        }
        if p.at_word("when") {
            let guard = p.start();
            p.bump();
            exprs::expr(p);
            guard.complete(p, GUARD);
        }
        p.no_lambda = saved;
    }
    m.complete(p, CASE_LABEL);
}

/// Whether a pattern, rather than a constant expression, starts here.
fn at_pattern(p: &Parser<'_>) -> bool {
    if matches!(p.current(), FINAL_KW | AT) {
        return true;
    }
    match la_type(p, p.pos()) {
        Some(end) => {
            (p.kind_at(end) == IDENT && !p.is_word_at(end, "when")) || p.kind_at(end) == L_PAREN
        }
        None => false,
    }
}
