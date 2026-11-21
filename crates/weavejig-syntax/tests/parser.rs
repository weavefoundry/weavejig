//! Tree-shape and error-recovery tests for the parser.

use weavejig_syntax::{SyntaxKind, SyntaxKind::*, parse};

/// Parses `body` as the statements of a method and returns the kinds of all
/// nodes inside the method's block, in preorder.
fn kinds_in_method(body: &str) -> Vec<SyntaxKind> {
    let text = format!("class C {{ void m() {{ {body} }} }}");
    let parse = parse(&text);
    assert!(
        parse.is_ok(),
        "{body}: {:?}\n{}",
        parse.errors(),
        parse.debug_tree()
    );
    let block = parse
        .syntax()
        .descendants()
        .find(|n| n.kind() == BLOCK)
        .unwrap();
    block.descendants().skip(1).map(|n| n.kind()).collect()
}

fn accepts(text: &str) {
    let parse = parse(text);
    assert!(
        parse.is_ok(),
        "{text}: {:?}\n{}",
        parse.errors(),
        parse.debug_tree()
    );
    assert_eq!(parse.syntax().text().to_string(), text);
}

fn rejects(text: &str) {
    let parse = parse(text);
    assert!(
        !parse.is_ok(),
        "{text} should not parse:\n{}",
        parse.debug_tree()
    );
    assert_eq!(parse.syntax().text().to_string(), text);
}

#[test]
fn declaration_or_expression() {
    assert_eq!(
        kinds_in_method("a = b;"),
        [EXPR_STMT, ASSIGN_EXPR, NAME_EXPR, NAME_EXPR]
    );
    assert_eq!(
        kinds_in_method("a b;"),
        [LOCAL_VAR_DECL, NAMED_TYPE, VARIABLE_DECLARATOR]
    );
    assert_eq!(
        kinds_in_method("a.b<c> d = e;"),
        [
            LOCAL_VAR_DECL,
            NAMED_TYPE,
            TYPE_ARGS,
            NAMED_TYPE,
            VARIABLE_DECLARATOR,
            NAME_EXPR
        ]
    );
    assert_eq!(
        kinds_in_method("a[i] = 1;"),
        [
            EXPR_STMT,
            ASSIGN_EXPR,
            ARRAY_ACCESS,
            NAME_EXPR,
            NAME_EXPR,
            LITERAL
        ]
    );
    assert_eq!(
        kinds_in_method("int[] a[];"),
        [
            LOCAL_VAR_DECL,
            ARRAY_TYPE,
            PRIMITIVE_TYPE,
            VARIABLE_DECLARATOR
        ]
    );
    assert_eq!(kinds_in_method("a < b;").first(), Some(&EXPR_STMT));
}

#[test]
fn generics_and_shifts() {
    assert_eq!(
        kinds_in_method("x = a >> b;"),
        [
            EXPR_STMT,
            ASSIGN_EXPR,
            NAME_EXPR,
            BINARY_EXPR,
            NAME_EXPR,
            NAME_EXPR
        ]
    );
    assert_eq!(
        kinds_in_method("x = a >>> b > c;"),
        [
            EXPR_STMT,
            ASSIGN_EXPR,
            NAME_EXPR,
            BINARY_EXPR,
            BINARY_EXPR,
            NAME_EXPR,
            NAME_EXPR,
            NAME_EXPR
        ]
    );
    assert_eq!(
        kinds_in_method("Map<K, List<V>> m;"),
        [
            LOCAL_VAR_DECL,
            NAMED_TYPE,
            TYPE_ARGS,
            NAMED_TYPE,
            NAMED_TYPE,
            TYPE_ARGS,
            NAMED_TYPE,
            VARIABLE_DECLARATOR
        ]
    );
    // `> >` with a space is not a shift.
    rejects("class C { int m() { return a > > b; } }");
}

#[test]
fn precedence() {
    // a + b * c  =>  a + (b * c)
    assert_eq!(
        kinds_in_method("x = a + b * c;")[3..],
        [BINARY_EXPR, NAME_EXPR, BINARY_EXPR, NAME_EXPR, NAME_EXPR]
    );
    // a * b + c  =>  (a * b) + c
    assert_eq!(
        kinds_in_method("x = a * b + c;")[3..],
        [BINARY_EXPR, BINARY_EXPR, NAME_EXPR, NAME_EXPR, NAME_EXPR]
    );
    // a - b - c  =>  (a - b) - c
    assert_eq!(
        kinds_in_method("x = a - b - c;")[3..],
        [BINARY_EXPR, BINARY_EXPR, NAME_EXPR, NAME_EXPR, NAME_EXPR]
    );
    // a = b = c  =>  a = (b = c)
    assert_eq!(
        kinds_in_method("a = b = c;"),
        [
            EXPR_STMT,
            ASSIGN_EXPR,
            NAME_EXPR,
            ASSIGN_EXPR,
            NAME_EXPR,
            NAME_EXPR
        ]
    );
}

#[test]
fn casts_lambdas_and_parens() {
    assert_eq!(
        kinds_in_method("x = (T) y;")[3..],
        [CAST_EXPR, NAMED_TYPE, NAME_EXPR]
    );
    assert_eq!(
        kinds_in_method("x = (int) -y;")[3..],
        [CAST_EXPR, PRIMITIVE_TYPE, PREFIX_EXPR, NAME_EXPR]
    );
    // `(a) - b` is a subtraction, not a cast of `-b`.
    assert_eq!(
        kinds_in_method("x = (a) - b;")[3..],
        [BINARY_EXPR, PAREN_EXPR, NAME_EXPR, NAME_EXPR]
    );
    assert_eq!(
        kinds_in_method("x = (a, b) -> a;")[3..],
        [LAMBDA_EXPR, LAMBDA_PARAMS, PARAM, PARAM, NAME_EXPR]
    );
    assert_eq!(
        kinds_in_method("x = a -> { };")[3..],
        [LAMBDA_EXPR, PARAM, BLOCK]
    );
    assert_eq!(
        kinds_in_method("x = (A & B) () -> 1;")[3..],
        [
            CAST_EXPR,
            INTERSECTION_TYPE,
            NAMED_TYPE,
            NAMED_TYPE,
            LAMBDA_EXPR,
            LAMBDA_PARAMS,
            LITERAL
        ]
    );
}

#[test]
fn calls_and_references() {
    assert_eq!(
        kinds_in_method("a.b(c).d;"),
        [
            EXPR_STMT,
            FIELD_ACCESS,
            METHOD_CALL,
            NAME_EXPR,
            ARG_LIST,
            NAME_EXPR
        ]
    );
    assert_eq!(
        kinds_in_method("x = List<String>::new;")[3..],
        [METHOD_REF, NAMED_TYPE, TYPE_ARGS, NAMED_TYPE]
    );
    assert_eq!(
        kinds_in_method("x = int[].class;")[3..],
        [CLASS_LITERAL, ARRAY_TYPE, PRIMITIVE_TYPE]
    );
    assert_eq!(
        kinds_in_method("this.<T>f();"),
        [
            EXPR_STMT,
            METHOD_CALL,
            THIS_EXPR,
            TYPE_ARGS,
            NAMED_TYPE,
            ARG_LIST
        ]
    );
    assert_eq!(
        kinds_in_method("x = new int[2][];")[3..],
        [ARRAY_CREATION_EXPR, PRIMITIVE_TYPE, DIM_EXPR, LITERAL]
    );
}

#[test]
fn switches_and_patterns() {
    accepts("class C { int m(Object o) { return switch (o) { case A a -> 1; default -> 2; }; } }");
    accepts(
        "class C { int m(Object o) { return switch (o) { \
         case Point(int x, var y) when x > 0 -> x; case null, default -> 0; }; } }",
    );
    accepts("class C { void m(E e) { switch (e) { case A, B: f(); break; default: } } }");
    accepts("class C { void m(E e) { switch (e) { case A -> f(); case B -> { } } } }");
    let kinds = kinds_in_method("if (o instanceof String s && s.isEmpty()) { }");
    assert!(kinds.contains(&TYPE_PATTERN), "{kinds:?}");
    let kinds = kinds_in_method("b = o instanceof String && c;");
    assert!(!kinds.contains(&TYPE_PATTERN), "{kinds:?}");
}

#[test]
fn annotated_array_dimensions() {
    accepts(
        "class C { int @A [] f; @B Object @A [] @A(1) [] m(String @A [] a, int @A ... b) { return null; } }",
    );
    accepts("class C { ThreadLocal<int @A []> t; int legacy @A [] = {}, other @A [] @B []; }");
    assert_eq!(
        kinds_in_method("int @A [] a = new int @A [3] @B [];"),
        [
            LOCAL_VAR_DECL,
            ARRAY_TYPE,
            PRIMITIVE_TYPE,
            ANNOTATION,
            NAME,
            VARIABLE_DECLARATOR,
            ARRAY_CREATION_EXPR,
            PRIMITIVE_TYPE,
            DIM_EXPR,
            ANNOTATION,
            NAME,
            LITERAL,
            ANNOTATION,
            NAME,
        ]
    );
    // Old-style brackets after the name, and casts.
    kinds_in_method("int a @A [] = {}; Object o = (Object @A []) a; for (int @A [] r : rows) {}");
    // An annotation that is not followed by brackets does not belong to the type.
    rejects("class C { int @A f; }");
}

#[test]
fn contextual_keywords_are_identifiers() {
    accepts("class C { int record, var, yield, sealed, permits, when, module, open; }");
    accepts("class C { void m() { var var = 1; record = 2; yield = 3; sealed.permits(); } }");
    accepts("class C { void m() { record R(int x) {} enum E { A } interface I {} } }");
    accepts("open module a.b { requires transitive java.base; exports a.b to c, d; }");
}

#[test]
fn rejects_invalid_code() {
    rejects("class C { void m() { int x = ; } }");
    rejects("class C { void m() { foo(; } }");
    rejects("class C { void m() { try { } } }");
    rejects("class { }");
    rejects("class C { void m() { return 1 } }");
    rejects("class C extends { }");
    rejects("int x = 1;");
    rejects("class C { void m() { String s = \"abc; } }");
    rejects("class C { void m() { x = #; } }");
}

#[test]
fn missing_tokens_are_reported_where_they_belong() {
    let text = "class C {\n  int f() {\n    int x = 1\n    return x;\n  }\n}\n";
    let parse = parse(text);
    let [error] = parse.errors() else {
        panic!("expected one error, got {:?}", parse.errors());
    };
    assert_eq!(error.message, "expected `;`");
    // Just after `1` on line 3, not at `return` on line 4.
    assert_eq!(
        u32::from(error.range.start()),
        text.find('1').unwrap() as u32 + 1
    );
    assert!(error.range.is_empty());
}

#[test]
fn recovers_from_garbage() {
    // Whatever the input, parsing terminates and loses no text.
    for text in [
        "}}}}",
        "class C { ) ) ) }",
        "@",
        "class C { void m() { (((( } }",
        "a b c d e",
        "",
    ] {
        let parse = parse(text);
        assert_eq!(parse.syntax().text().to_string(), text);
    }
}

#[test]
fn deep_nesting_does_not_overflow() {
    let depth = 5_000;
    let text = format!(
        "class C {{ int x = {}1{}; }}",
        "(".repeat(depth),
        ")".repeat(depth)
    );
    let parse = parse(&text);
    assert!(!parse.is_ok());
    assert_eq!(parse.syntax().text().to_string(), text);

    let text = format!("class C {{ void m() {{ {} }} }}", "if (a) ".repeat(depth));
    assert!(!weavejig_syntax::parse(&text).is_ok());

    // Long chains are loops, not recursion, so they are fine at any length.
    let text = format!("class C {{ int x = 1{}; }}", " + 1".repeat(depth));
    assert!(weavejig_syntax::parse(&text).is_ok());
}
