//! The Java grammar, as a hand-written recursive-descent parser.
//!
//! Each function parses one construct and wraps it in a node. Functions that
//! loop must consume at least one token per iteration; where a sub-parser
//! might not, the loop checks for progress and skips a token itself.

mod exprs;
mod stmts;
mod types;

use crate::SyntaxKind::{self, *};
use crate::parser::{CompletedMarker, Marker, Parser};

pub(crate) fn compilation_unit(p: &mut Parser<'_>) {
    let m = p.start();
    while !p.at_end() {
        let before = p.pos();
        top_level_item(p);
        if p.pos() == before {
            p.err_and_bump("expected a class, interface, enum, or record");
        }
    }
    m.complete(p, COMPILATION_UNIT);
}

fn top_level_item(p: &mut Parser<'_>) {
    match p.current() {
        SEMI => {
            let m = p.start();
            p.bump();
            m.complete(p, EMPTY_DECL);
        }
        IMPORT_KW => {
            let m = p.start();
            p.bump();
            p.eat(STATIC_KW);
            name(p);
            if p.at(DOT) && p.nth(1) == STAR {
                p.bump();
                p.bump();
            }
            p.expect(SEMI);
            m.complete(p, IMPORT_DECL);
        }
        _ => {
            let m = p.start();
            let has_modifiers = modifiers(p, true);
            if p.at(PACKAGE_KW) {
                p.bump();
                name(p);
                p.expect(SEMI);
                m.complete(p, PACKAGE_DECL);
            } else if at_module_decl(p) {
                module_decl(p, m);
            } else if at_type_decl(p) {
                type_decl(p, m);
            } else {
                p.error("expected a class, interface, enum, or record");
                if has_modifiers {
                    m.complete(p, ERROR);
                } else {
                    m.abandon(p);
                }
            }
        }
    }
}

fn at_module_decl(p: &Parser<'_>) -> bool {
    let i = if p.at_word("open") {
        p.pos() + 1
    } else {
        p.pos()
    };
    p.is_word_at(i, "module") && p.kind_at(i + 1) == IDENT
}

/// Module declarations are parsed loosely: each directive is a run of tokens
/// up to its `;`.
fn module_decl(p: &mut Parser<'_>, m: Marker) {
    if p.at_word("open") {
        p.bump();
    }
    p.bump(); // `module`
    name(p);
    if p.expect(L_BRACE) {
        while !p.at(R_BRACE) && !p.at_end() {
            let directive = p.start();
            while !p.at(SEMI) && !p.at(R_BRACE) && !p.at_end() {
                p.bump();
            }
            p.expect(SEMI);
            directive.complete(p, MODULE_DIRECTIVE);
        }
        p.expect(R_BRACE);
    }
    m.complete(p, MODULE_DECL);
}

/// A possibly qualified name: `a.b.c`.
fn name(p: &mut Parser<'_>) {
    let m = p.start();
    p.expect(IDENT);
    while p.at(DOT) && p.nth(1) == IDENT {
        p.bump();
        p.bump();
    }
    m.complete(p, NAME);
}

/// Whether the current token starts a modifier (an annotation or a modifier
/// keyword). `in_member` distinguishes class-body context, where `default`
/// and `synchronized` are modifiers, from statement context, where they
/// begin statements or switch labels.
fn at_modifier(p: &Parser<'_>, in_member: bool) -> bool {
    match p.current() {
        AT => p.nth(1) != INTERFACE_KW,
        DEFAULT_KW => in_member && !matches!(p.nth(1), COLON | ARROW),
        SYNCHRONIZED_KW => in_member,
        IDENT => at_sealed(p) || at_non_sealed(p),
        kind => kind.is_modifier_keyword(),
    }
}

/// `sealed` is only a modifier when a declaration can follow it.
fn at_sealed(p: &Parser<'_>) -> bool {
    p.at_word("sealed")
        && (matches!(p.nth(1), CLASS_KW | INTERFACE_KW | AT)
            || p.nth(1).is_modifier_keyword()
            || p.is_word_at(p.pos() + 1, "non"))
}

fn at_non_sealed(p: &Parser<'_>) -> bool {
    let i = p.pos();
    p.at_word("non")
        && p.nth(1) == MINUS
        && p.is_word_at(i + 2, "sealed")
        && p.joint_at(i)
        && p.joint_at(i + 1)
}

/// Parses annotations and modifier keywords. Returns whether there were any.
fn modifiers(p: &mut Parser<'_>, in_member: bool) -> bool {
    if !at_modifier(p, in_member) {
        return false;
    }
    let m = p.start();
    while at_modifier(p, in_member) {
        if p.at(AT) {
            annotation(p);
        } else if at_non_sealed(p) {
            p.bump();
            p.bump();
            p.bump();
        } else {
            p.bump();
        }
    }
    m.complete(p, MODIFIERS);
    true
}

fn annotation(p: &mut Parser<'_>) {
    if !p.enter() {
        return;
    }
    let m = p.start();
    p.bump(); // `@`
    name(p);
    if p.at(L_PAREN) {
        let args = p.start();
        p.bump();
        if p.at(IDENT) && p.nth(1) == EQ {
            loop {
                let pair = p.start();
                p.expect(IDENT);
                p.expect(EQ);
                element_value(p);
                pair.complete(p, ANNOTATION_PAIR);
                if !p.eat(COMMA) {
                    break;
                }
            }
        } else if !p.at(R_PAREN) {
            element_value(p);
        }
        p.expect(R_PAREN);
        args.complete(p, ANNOTATION_ARGS);
    }
    m.complete(p, ANNOTATION);
    p.leave();
}

fn element_value(p: &mut Parser<'_>) {
    match p.current() {
        AT => annotation(p),
        L_BRACE => exprs::array_init(p),
        _ => {
            exprs::conditional(p);
        }
    }
}

fn at_record_decl(p: &Parser<'_>) -> bool {
    p.at_word("record") && p.nth(1) == IDENT && matches!(p.nth(2), L_PAREN | LT)
}

fn at_type_decl(p: &Parser<'_>) -> bool {
    match p.current() {
        CLASS_KW | INTERFACE_KW | ENUM_KW => true,
        AT => p.nth(1) == INTERFACE_KW,
        IDENT => at_record_decl(p),
        _ => false,
    }
}

/// Parses a type declaration whose modifiers are already inside `m`. The
/// caller must have checked [`at_type_decl`].
fn type_decl(p: &mut Parser<'_>, m: Marker) -> CompletedMarker {
    let kind = match p.current() {
        CLASS_KW => {
            p.bump();
            p.expect(IDENT);
            if p.at(LT) {
                types::type_params(p);
            }
            type_list_clause(p, EXTENDS_CLAUSE);
            type_list_clause(p, IMPLEMENTS_CLAUSE);
            type_list_clause(p, PERMITS_CLAUSE);
            class_body(p);
            CLASS_DECL
        }
        INTERFACE_KW => {
            p.bump();
            p.expect(IDENT);
            if p.at(LT) {
                types::type_params(p);
            }
            type_list_clause(p, EXTENDS_CLAUSE);
            type_list_clause(p, PERMITS_CLAUSE);
            class_body(p);
            INTERFACE_DECL
        }
        ENUM_KW => {
            p.bump();
            p.expect(IDENT);
            type_list_clause(p, IMPLEMENTS_CLAUSE);
            enum_body(p);
            ENUM_DECL
        }
        AT => {
            p.bump();
            p.bump(); // `interface`
            p.expect(IDENT);
            class_body(p);
            ANNOTATION_TYPE_DECL
        }
        _ => {
            p.bump(); // `record`
            p.expect(IDENT);
            if p.at(LT) {
                types::type_params(p);
            }
            param_list(p);
            type_list_clause(p, IMPLEMENTS_CLAUSE);
            class_body(p);
            RECORD_DECL
        }
    };
    m.complete(p, kind)
}

/// An optional `extends`, `implements`, `permits` or `throws` clause.
fn type_list_clause(p: &mut Parser<'_>, kind: SyntaxKind) {
    let at_clause = match kind {
        EXTENDS_CLAUSE => p.at(EXTENDS_KW),
        IMPLEMENTS_CLAUSE => p.at(IMPLEMENTS_KW),
        THROWS_CLAUSE => p.at(THROWS_KW),
        _ => p.at_word("permits"),
    };
    if !at_clause {
        return;
    }
    let m = p.start();
    p.bump();
    loop {
        types::type_(p);
        if !p.eat(COMMA) {
            break;
        }
    }
    m.complete(p, kind);
}

fn class_body(p: &mut Parser<'_>) {
    if !p.at(L_BRACE) {
        p.error("expected `{`");
        return;
    }
    if !p.enter() {
        return;
    }
    let m = p.start();
    p.bump();
    members(p);
    p.expect(R_BRACE);
    m.complete(p, CLASS_BODY);
    p.leave();
}

fn enum_body(p: &mut Parser<'_>) {
    if !p.at(L_BRACE) {
        p.error("expected `{`");
        return;
    }
    if !p.enter() {
        return;
    }
    let m = p.start();
    p.bump();
    while p.at(IDENT) || p.at(AT) {
        let constant = p.start();
        modifiers(p, true);
        p.expect(IDENT);
        if p.at(L_PAREN) {
            exprs::arg_list(p);
        }
        if p.at(L_BRACE) {
            class_body(p);
        }
        constant.complete(p, ENUM_CONSTANT);
        if !p.eat(COMMA) {
            break;
        }
    }
    if p.eat(SEMI) {
        members(p);
    }
    if !p.at(R_BRACE) {
        p.error("expected `,`, `;` or `}`");
        members(p);
    }
    p.expect(R_BRACE);
    m.complete(p, CLASS_BODY);
    p.leave();
}

fn members(p: &mut Parser<'_>) {
    while !p.at(R_BRACE) && !p.at_end() {
        let before = p.pos();
        member(p);
        if p.pos() == before {
            p.err_and_bump("expected a field, method, or nested type");
        }
    }
}

fn member(p: &mut Parser<'_>) {
    if p.at(SEMI) {
        let m = p.start();
        p.bump();
        m.complete(p, EMPTY_DECL);
        return;
    }
    let m = p.start();
    let has_modifiers = modifiers(p, true);
    if p.at(L_BRACE) {
        stmts::block(p);
        m.complete(p, INITIALIZER_BLOCK);
        return;
    }
    if at_type_decl(p) {
        type_decl(p, m);
        return;
    }
    let has_type_params = p.at(LT);
    if has_type_params {
        types::type_params(p);
    }
    if p.at(IDENT) && p.nth(1) == L_PAREN {
        p.bump();
        param_list(p);
        type_list_clause(p, THROWS_CLAUSE);
        stmts::block(p);
        m.complete(p, CONSTRUCTOR_DECL);
    } else if p.at(IDENT) && p.nth(1) == L_BRACE {
        // Compact canonical constructor of a record.
        p.bump();
        stmts::block(p);
        m.complete(p, CONSTRUCTOR_DECL);
    } else if types::at_type_start(p) || p.at(VOID_KW) {
        types::type_or_void(p);
        if p.at(IDENT) && p.nth(1) == L_PAREN {
            p.bump();
            param_list(p);
            dims(p);
            type_list_clause(p, THROWS_CLAUSE);
            if p.at(DEFAULT_KW) {
                let default = p.start();
                p.bump();
                element_value(p);
                default.complete(p, DEFAULT_VALUE);
            }
            if p.at(L_BRACE) {
                stmts::block(p);
            } else {
                p.expect(SEMI);
            }
            m.complete(p, METHOD_DECL);
        } else {
            declarators(p);
            p.expect(SEMI);
            m.complete(p, FIELD_DECL);
        }
    } else {
        p.error("expected a field, method, or nested type");
        if has_modifiers || has_type_params {
            m.complete(p, ERROR);
        } else {
            m.abandon(p);
        }
    }
}

fn declarators(p: &mut Parser<'_>) {
    loop {
        let m = p.start();
        p.expect(IDENT);
        dims(p);
        if p.eat(EQ) {
            if p.at(L_BRACE) {
                exprs::array_init(p);
            } else {
                exprs::expr(p);
            }
        }
        m.complete(p, VARIABLE_DECLARATOR);
        if !p.eat(COMMA) {
            break;
        }
    }
}

/// Array brackets after a name, as in `int a[]`.
fn dims(p: &mut Parser<'_>) {
    while types::at_dim(p) {
        types::dim(p);
    }
}

fn param_list(p: &mut Parser<'_>) {
    let m = p.start();
    p.expect(L_PAREN);
    if !p.at(R_PAREN) {
        loop {
            param(p);
            if !p.eat(COMMA) {
                break;
            }
        }
    }
    p.expect(R_PAREN);
    m.complete(p, PARAM_LIST);
}

/// A formal parameter, record component, or receiver parameter.
fn param(p: &mut Parser<'_>) {
    let m = p.start();
    modifiers(p, false);
    types::type_(p);
    while p.at(AT) {
        annotation(p); // annotations on a varargs ellipsis
    }
    p.eat(ELLIPSIS);
    if !p.eat(THIS_KW) {
        p.expect(IDENT);
        if p.at(DOT) && p.nth(1) == THIS_KW {
            p.bump();
            p.bump();
        }
        dims(p);
    }
    m.complete(p, PARAM);
}
