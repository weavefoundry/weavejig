//! Behavioural tests for the formatter.

use std::fs;
use std::path::{Path, PathBuf};

use weavejig_formatter::{FormatError, FormatOptions, Style, format};

fn fmt(text: &str) -> String {
    format(text, &FormatOptions::default()).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

/// Formats `body` as the statements of a method and returns just those lines.
fn fmt_statements(body: &str) -> String {
    let formatted = fmt(&format!("class C {{ void m() {{ {body} }} }}"));
    let lines: Vec<&str> = formatted.lines().collect();
    lines[2..lines.len() - 2]
        .iter()
        .map(|line| line.strip_prefix("    ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn smoke_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/smoke");
    let mut files: Vec<PathBuf> = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "java"))
        .collect();
    files.sort();
    assert!(files.len() >= 8);
    files
}

#[test]
fn smoke_corpus_formats_and_is_idempotent() {
    for path in smoke_files() {
        let text = fs::read_to_string(&path).unwrap();
        let once = format(&text, &FormatOptions::default())
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let twice = fmt(&once);
        assert_eq!(once, twice, "{} is not stable", path.display());
        assert!(
            once.lines().all(|line| line.chars().count() <= 100),
            "{}",
            path.display()
        );
        assert!(
            !once.lines().any(|line| line.ends_with(' ')),
            "{}",
            path.display()
        );
    }
}

#[test]
fn already_formatted_files_are_unchanged() {
    // The smoke files that google-java-format leaves as they are.
    for name in ["Basics.java", "package-info.java"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/smoke")
            .join(name);
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(fmt(&text), text, "{name}");
    }
}

#[test]
fn spacing() {
    assert_eq!(fmt_statements("x=a+b*-c;"), "x = a + b * -c;");
    assert_eq!(
        fmt_statements("x = - -a + + +b - --c;"),
        "x = - -a + + +b - --c;"
    );
    assert_eq!(fmt_statements("x = a>>b>>>c<<d;"), "x = a >> b >>> c << d;");
    assert_eq!(
        fmt_statements("x = (int)y + (T)(z);"),
        "x = (int) y + (T) (z);"
    );
    assert_eq!(
        fmt_statements("List < Map<K,V> > m = f();"),
        "List<Map<K, V>> m = f();"
    );
    assert_eq!(
        fmt_statements("a . b ( c , d ) [ 0 ] ++;"),
        "a.b(c, d)[0]++;"
    );
    assert_eq!(fmt_statements("r = Foo :: bar;"), "r = Foo::bar;");
    assert_eq!(fmt_statements("r = Foo.< T >bar( );"), "r = Foo.<T>bar();");
    assert_eq!(fmt_statements("f = ( a,b )->a+b;"), "f = (a, b) -> a + b;");
    assert_eq!(
        fmt_statements("int [ ] a = new int [ ] { 1,2 };"),
        "int[] a = new int[] {1, 2};"
    );
    assert_eq!(fmt_statements("for(;;){}"), "for (; ; ) {}");
    assert_eq!(
        fmt_statements("for(String s:xs){}"),
        "for (String s : xs) {}"
    );
    assert_eq!(fmt_statements("assert x:\"m\";"), "assert x : \"m\";");
    assert_eq!(
        fmt_statements("b = x instanceof Foo f&&!f.y();"),
        "b = x instanceof Foo f && !f.y();"
    );
    assert_eq!(
        fmt_statements("synchronized(this){}"),
        "synchronized (this) {\n}"
    );
    assert_eq!(
        fmt_statements("Object o=new Object(){};"),
        "Object o = new Object() {};"
    );
}

#[test]
fn declarations() {
    assert_eq!(
        fmt("public   final class A<T>extends B<T>implements C,D{}"),
        "public final class A<T> extends B<T> implements C, D {}\n"
    );
    assert_eq!(
        fmt("class A{static<T>T f(T...ts)throws E{return null;}}"),
        "class A {\n  static <T> T f(T... ts) throws E {\n    return null;\n  }\n}\n"
    );
    assert_eq!(
        fmt("sealed interface S permits A,B{}  non-sealed class A implements S{}"),
        "sealed interface S permits A, B {}\n\nnon-sealed class A implements S {}\n"
    );
    assert_eq!(
        fmt("@interface A{int v()default 1;}"),
        "@interface A {\n  int v() default 1;\n}\n"
    );
    assert_eq!(
        fmt("record P<T>(T a,int b){}"),
        "record P<T>(T a, int b) {}\n"
    );
    assert_eq!(fmt("enum E{A,B;}"), "enum E {\n  A,\n  B;\n}\n");
}

#[test]
fn annotations_go_above_declarations_but_stay_inline_elsewhere() {
    assert_eq!(
        fmt("@A @B(1) class C { @A int f; @B(1) int g; @A void m(@A int p) { @A int v; } }"),
        "\
@A
@B(1)
class C {
  @A int f;

  @B(1)
  int g;

  @A
  void m(@A int p) {
    @A int v;
  }
}
"
    );
}

#[test]
fn imports_are_sorted_static_first() {
    assert_eq!(
        fmt(
            "import b.B;\nimport static z.Z.z;\nimport a.A;\nimport a.A;\nimport static a.A.*;\nclass C {A a; B b; int x = z;}"
        ),
        "import static a.A.*;\nimport static z.Z.z;\n\nimport a.A;\nimport b.B;\n\nclass C {\n  A a;\n  B b;\n  int x = z;\n}\n"
    );
}

#[test]
fn unused_imports_are_removed() {
    assert_eq!(
        fmt("package p;\n\n\
             import a.Unused;\n\
             import a.Used;\n\
             import a.Member;\n\
             import a.Declared;\n\
             import a.Linked;\n\
             import a.Thrown;\n\
             import a.Param;\n\
             import a.Code;\n\
             import a.Local;\n\
             import java.lang.String;\n\
             import p.Same;\n\
             import java.util.*;\n\
             import static a.S.used;\n\
             import static a.S.unused;\n\n\
             /**\n * {@link Linked} {@link Foo#bar(Param, int)} {@code Code}\n *\n * @throws Thrown\n */\n\
             class C {\n  Used u;\n  int Declared;\n  Object o = x.Member;\n  String s;\n  Same t;\n\
             void f() {\n    /** {@link Local} */\n    int z = used();\n  }\n}\n"),
        "package p;\n\n\
         import static a.S.used;\n\n\
         import a.Linked;\n\
         import a.Param;\n\
         import a.Thrown;\n\
         import a.Used;\n\
         import java.util.*;\n\n\
         /**\n * {@link Linked} {@link Foo#bar(Param, int)} {@code Code}\n *\n * @throws Thrown\n */\n\
         class C {\n  Used u;\n  int Declared;\n  Object o = x.Member;\n  String s;\n  Same t;\n\n\
         \x20 void f() {\n    /** {@link Local} */\n    int z = used();\n  }\n}\n"
    );
    assert_eq!(
        fmt("package p;\n\nimport a.A;\nimport b.B;\n\nclass C {}\n"),
        "package p;\n\nclass C {}\n"
    );
    assert_eq!(fmt("import a.A;\n\nclass C {}\n"), "class C {}\n");
}

#[test]
fn comments_travel_with_the_import_above_them() {
    // A `//` comment on the lines below an import moves with it.
    assert_eq!(
        fmt("import c.C;\n// about B\nimport b.B;\nimport a.A;\n\nclass K { A a; B b; C c; }\n"),
        "import a.A;\nimport b.B;\nimport c.C;\n\n// about B\n\nclass K {\n  A a;\n  B b;\n  C c;\n}\n"
    );
    // So does that of a removed import, and a comment on its line.
    assert_eq!(
        fmt("import b.B;\nimport x.X; // tail\nimport a.A;\nclass K { A a; B b; }\n"),
        "import a.A;\nimport b.B;\n\n// tail\n\nclass K {\n  A a;\n  B b;\n}\n"
    );
    assert_eq!(
        fmt("// top\nimport a.Unused; // gone\nimport b.B;\nclass K { B b; }\n"),
        "// top\n// gone\nimport b.B;\n\nclass K {\n  B b;\n}\n"
    );
    // A static import's comments stay above the blank line.
    assert_eq!(
        fmt(
            "import b.B;\nimport static z.Z.z;\n// about a\nimport a.A;\nclass K { A a; B b; int q = z; }\n"
        ),
        "import static z.Z.z;\n// about a\n\nimport a.A;\nimport b.B;\n\nclass K {\n  A a;\n  B b;\n  int q = z;\n}\n"
    );
}

#[test]
fn modifiers_are_reordered() {
    assert_eq!(
        fmt("class M {\n\
             final static public int A = 1;\n\
             static @Deprecated final int B = 2;\n\
             final /* c */ static int E = 5;\n\
             static final @A volatile transient int x;\n\
             synchronized public static void f() {}\n\
             final static sealed class R {}\n\
             non-sealed static public class Y extends R {}\n\
             static protected abstract void g();\n\
             }\n"),
        "class M {\n\
         \x20 public static final int A = 1;\n\
         \x20 static @Deprecated final int B = 2;\n\
         \x20 static /* c */ final int E = 5;\n\
         \x20 static final @A transient volatile int x;\n\n\
         \x20 public static synchronized void f() {}\n\n\
         \x20 static sealed final class R {}\n\n\
         \x20 public static non-sealed class Y extends R {}\n\n\
         \x20 protected abstract static void g();\n\
         }\n"
    );
}

#[test]
fn blank_lines() {
    // Kept between statements and after the opening brace (at most one),
    // dropped before the closing brace.
    assert_eq!(
        fmt_statements("\n\n a();\n\n\n\n b();\n c();\n\n"),
        "\na();\n\nb();\nc();"
    );
    // Members are separated by a blank line; fields may be packed.
    assert_eq!(
        fmt("class C { int a; int b;\n\n int c; void m() {} void n() {} }"),
        "class C {\n  int a;\n  int b;\n\n  int c;\n\n  void m() {}\n\n  void n() {}\n}\n"
    );
}

#[test]
fn comments_are_kept() {
    let text = "\
// file header

/** Doc for C. */
class C { // after brace
  int a; // trailing
  /* before b */ int b;

  // own line

  void m(int x /* inline */, int y) {
    // only a comment
  }

  void n() {
    f(); // call
    // last in block
  }
  // last in class
}
// end of file
";
    assert_eq!(fmt(text), text);
}

#[test]
fn line_comment_never_swallows_code() {
    assert_eq!(
        fmt_statements("f(a, // first\n b);"),
        "f(\n    a, // first\n    b);"
    );
    assert_eq!(
        fmt_statements("int x = // why\n 1;"),
        "int x = // why\n    1;"
    );
}

#[test]
fn long_lines_wrap() {
    let long_call = format!("result = target.method({});", ["argument"; 12].join(", "));
    let formatted = fmt_statements(&long_call);
    assert!(formatted.lines().count() > 1, "{formatted}");
    assert!(
        formatted.lines().all(|line| line.len() <= 96),
        "{formatted}"
    );
}

#[test]
fn block_lambdas() {
    // The only argument: stays on the line of the call.
    assert_eq!(
        fmt_statements("run(() -> { f(); });"),
        "run(() -> {\n  f();\n});"
    );
    // One of several arguments: every argument gets its own line.
    assert_eq!(
        fmt_statements("run(a, () -> { f(); });"),
        "run(\n    a,\n    () -> {\n      f();\n    });"
    );
    // Assigned: starts on a continuation line.
    assert_eq!(
        fmt_statements("Runnable r = () -> { f(); };"),
        "Runnable r =\n    () -> {\n      f();\n    };"
    );
}

#[test]
fn aosp_style_uses_wider_indents() {
    let options = FormatOptions { style: Style::Aosp };
    assert_eq!(
        format("class C { void m() { f(); } }", &options).unwrap(),
        "class C {\n    void m() {\n        f();\n    }\n}\n"
    );
}

#[test]
fn invalid_java_is_rejected() {
    let error = format(
        "class C { void m() { int x = ; } }",
        &FormatOptions::default(),
    );
    assert!(matches!(error, Err(FormatError::Syntax(_))), "{error:?}");
}

#[test]
fn empty_and_comment_only_files() {
    assert_eq!(fmt(""), "");
    assert_eq!(fmt("\n\n"), "");
    assert_eq!(fmt("// just a comment"), "// just a comment\n");
    assert_eq!(fmt("class C {}\n\n\n"), "class C {}\n");
}

#[test]
fn long_chains_do_not_overflow_the_stack() {
    let sum = format!("class C {{ int x = 1{}; }}", " + 1".repeat(3_000));
    assert!(fmt(&sum).lines().count() > 3_000);
    let calls = format!("class C {{ Object x = a{}; }}", ".b()".repeat(3_000));
    assert!(fmt(&calls).lines().count() > 3_000);
}

#[test]
fn annotated_array_brackets_are_set_off() {
    assert_eq!(
        fmt_statements(
            "int @A[] a = new int @A[3] @B(1)[]; Object o = (Object @A[]) a; int  b  @A[];"
        ),
        "int @A [] a = new int @A [3] @B(1) [];\nObject o = (Object @A []) a;\nint b @A [];"
    );
    assert_eq!(
        fmt("class C { void m(String @A... rest, int[] plain) {} }"),
        "class C {\n  void m(String @A ... rest, int[] plain) {}\n}\n"
    );
}

#[test]
fn comment_above_a_statement_does_not_wrap_it() {
    assert_eq!(
        fmt_statements("a();\n// why\nassertThat(a).isEqualTo(b);\n/* also */\nx = y;"),
        "a();\n// why\nassertThat(a).isEqualTo(b);\n/* also */\nx = y;"
    );
}

#[test]
fn block_comments_keep_their_shape() {
    // Commented-out code keeps its relative indentation, and later lines
    // that all start with `*` line up one column in.
    let expected = "\
class C {
  void m() {
    /*
        if (x) {
            foo();
        }
    */
    g();
    /* one
     * two
     */
    h();
  }
}
";
    let input = "\
class C {
void m() {
      /*
          if (x) {
              foo();
          }
      */
    g();
  /* one
         * two
   */
    h();
  }
}
";
    assert_eq!(fmt(input), expected);
    assert_eq!(fmt(expected), expected);
}

#[test]
fn long_line_comments_with_wide_characters_wrap() {
    // Each `n` puts a different character at the wrap column; one of them
    // used to slice a two-byte `é` in half and panic.
    for n in 85..96 {
        let comment = format!("// {} {}", "a".repeat(n), "é".repeat(30));
        let formatted = fmt(&format!(
            "class C {{\n  void m() {{\n    {comment}\n    g();\n  }}\n}}\n"
        ));
        // Past 93, the run of `a` alone overflows: no space to break at.
        if n <= 93 {
            assert!(
                formatted.contains(&format!("    // {}\n", "é".repeat(30))),
                "{formatted}"
            );
        }
        assert_eq!(fmt(&formatted), formatted);
    }
}
