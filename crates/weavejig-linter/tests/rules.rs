//! Behaviour of each lint rule and its fix.

use weavejig_linter::{RuleSet, SYNTAX_ERROR, fix_all, lint};
use weavejig_syntax::LineIndex;

/// Lints `text` with one rule and returns the 1-based lines it reports.
fn lines(rule: &str, text: &str) -> Vec<u32> {
    let rules = RuleSet::from_selection(Some(&[rule.to_string()]), &[]).unwrap();
    let index = LineIndex::new(text);
    lint(text, &rules)
        .iter()
        .map(|d| {
            assert_eq!(d.rule, rule);
            index.line_col(text, d.range.start()).line
        })
        .collect()
}

/// Applies one rule's fixes and returns the fixed text.
fn fixed(rule: &str, text: &str) -> String {
    let rules = RuleSet::from_selection(Some(&[rule.to_string()]), &[]).unwrap();
    fix_all(text, &rules).text
}

#[test]
fn upper_ell() {
    let text = "class C {\n  long a = 1l;\n  long b = 0xFFl + 2L;\n  int l = 3;\n}\n";
    assert_eq!(lines("upper-ell", text), [2, 3]);
    assert_eq!(
        fixed("upper-ell", text),
        "class C {\n  long a = 1L;\n  long b = 0xFFL + 2L;\n  int l = 3;\n}\n"
    );
}

#[test]
fn avoid_star_import() {
    let text = "import java.util.*;\nimport static java.lang.Math.*;\nimport java.util.List;\n";
    assert_eq!(lines("avoid-star-import", text), [1, 2]);
}

#[test]
fn empty_statement() {
    let text = "class C {\n  ;\n  void m() {\n    ;\n    f();;\n    while (g());\n  }\n}\n";
    // The class-level `;` on line 2 is an empty declaration, not a statement.
    assert_eq!(lines("empty-statement", text), [4, 5, 6]);
    // The loop body is left alone: deleting it would change the loop.
    assert_eq!(
        fixed("empty-statement", text),
        "class C {\n  ;\n  void m() {\n    \n    f();\n    while (g());\n  }\n}\n"
    );
}

#[test]
fn need_braces() {
    let text = "\
class C {
  void m(int n) {
    if (n > 0) a();
    else b();
    if (n > 1) {
      a();
    } else if (n > 2) {
      b();
    }
    for (int i = 0; i < n; i++) a();
    for (int x : xs) a();
    while (n-- > 0);
    do a(); while (n < 0);
    while (n > 0) {
      a();
    }
  }
}
";
    assert_eq!(lines("need-braces", text), [3, 4, 10, 11, 12, 13]);
    let fixed = fixed("need-braces", text);
    assert!(
        fixed.contains("if (n > 0) { a(); }\n    else { b(); }"),
        "{fixed}"
    );
    assert!(fixed.contains("while (n-- > 0) {}"), "{fixed}");
    assert!(fixed.contains("do { a(); } while (n < 0);"), "{fixed}");
    assert!(lines("need-braces", &fixed).is_empty());
}

#[test]
fn need_braces_fixes_nested_bodies() {
    let text = "class C { void m() { if (a) if (b) c(); else d(); } }";
    let fixed = fixed("need-braces", text);
    assert_eq!(
        fixed,
        "class C { void m() { if (a) { if (b) { c(); } else { d(); } } } }"
    );
}

#[test]
fn modifier_order() {
    let text = "\
class C {
  final static int A = 1;
  static public void m() {}
  public static final int B = 2;
  @Deprecated public static int c;
  public @Deprecated static int d;
  public static @Nullable String e;
  synchronized static final private void n() {}
  private final @NonNull @Valid Foo f;
  public @A @B static int g;
}
";
    assert_eq!(lines("modifier-order", text), [2, 3, 6, 8, 10]);
    let fixed = fixed("modifier-order", text);
    assert!(fixed.contains("static final int A"), "{fixed}");
    assert!(fixed.contains("public static void m"), "{fixed}");
    assert!(
        fixed.contains("private static final synchronized void n"),
        "{fixed}"
    );
    // A misplaced annotation has no automatic fix.
    assert_eq!(lines("modifier-order", &fixed), [6, 10]);
}

#[test]
fn modifier_order_handles_non_sealed() {
    let text = "final class A {}\nnon-sealed static class B {}\nstatic non-sealed class C {}\n";
    assert_eq!(lines("modifier-order", text), [2]);
    assert!(fixed("modifier-order", text).contains("static non-sealed class B"));
}

#[test]
fn array_type_style() {
    let text = "\
class C {
  int a[];
  int[] b;
  int c[], d;
  int[] e[];
  void m(String args[], int... rest) {
    for (int xs[] : grid) {}
  }
  int n()[] { return null; }
}
";
    assert_eq!(lines("array-type-style", text), [2, 4, 5, 6, 7, 9]);
    let fixed = fixed("array-type-style", text);
    assert!(fixed.contains("int[] a;"), "{fixed}");
    assert!(fixed.contains("int[][] e;"), "{fixed}");
    assert!(fixed.contains("String[] args,"), "{fixed}");
    assert!(fixed.contains("for (int[] xs : grid)"), "{fixed}");
    // With several declarators, moving the brackets would change `d`.
    assert!(fixed.contains("int c[], d;"), "{fixed}");
}

#[test]
fn syntax_errors_replace_rule_diagnostics() {
    let text = "class C { long a = 1l; void m() { if (x) y() } }";
    let diagnostics = lint(text, &RuleSet::all());
    assert!(!diagnostics.is_empty());
    assert!(diagnostics.iter().all(|d| d.rule == SYNTAX_ERROR));
    // Nothing is fixed in a file that does not parse.
    assert_eq!(fix_all(text, &RuleSet::all()).text, text);
}

#[test]
fn rule_selection() {
    assert!(RuleSet::from_selection(Some(&["no-such-rule".to_string()]), &[]).is_err());
    assert!(RuleSet::from_selection(None, &["no-such-rule".to_string()]).is_err());
    let rules = RuleSet::from_selection(None, &["upper-ell".to_string()]).unwrap();
    assert!(lint("class C { long a = 1l; }", &rules).is_empty());
}

#[test]
fn fixing_everything_at_once_still_parses() {
    let text = "\
import java.util.*;
class C {
  final static long A = 10l;
  int xs[];
  void m(int n) {
    if (n > 0) xs[0]++;
    else for (;;) ;
    ;
  }
}
";
    let outcome = fix_all(text, &RuleSet::all());
    assert!(
        weavejig_syntax::parse(&outcome.text).is_ok(),
        "{}",
        outcome.text
    );
    assert!(outcome.fixed >= 5, "{outcome:?}");
    // Only the unfixable wildcard import is left.
    let left: Vec<_> = outcome.remaining.iter().map(|d| d.rule).collect();
    assert_eq!(left, ["avoid-star-import"]);
}
