//! Behaviour of each lint rule and its fix.

use std::path::Path;

use weavejig_linter::{
    RawValue, RuleSet, SYNTAX_ERROR, Selection, Source, SuppressionNames, all_rules, fix_all, lint,
};
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
  public @A void g() {}
  public @A String h() { return null; }
  public @A class Inner {}
}
";
    // After a keyword, an annotation is out of place on a class and on a
    // method that returns nothing; elsewhere it may be annotating the type.
    assert_eq!(lines("modifier-order", text), [2, 3, 8, 10, 12]);
    let fixed = fixed("modifier-order", text);
    assert!(fixed.contains("static final int A"), "{fixed}");
    assert!(fixed.contains("public static void m"), "{fixed}");
    assert!(
        fixed.contains("private static final synchronized void n"),
        "{fixed}"
    );
    // A misplaced annotation has no automatic fix.
    assert_eq!(lines("modifier-order", &fixed), [10, 12]);
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
    assert!(lint("class C { long a = 1l; }\n", &rules).is_empty());

    // Without a selection, the rules that are opt-in stay off.
    let defaults = RuleSet::defaults();
    assert!(defaults.meta("need-braces").is_some());
    assert!(defaults.meta("magic-number").is_none());
    assert!(defaults.len() < RuleSet::all().len());
    let select = |select: Option<&[&str]>, extend: &[&str], ignore: &[&str]| {
        let strings = |ids: &[&str]| ids.iter().map(|id| id.to_string()).collect::<Vec<_>>();
        RuleSet::new(&Selection {
            select: select.map(strings),
            extend_select: strings(extend),
            ignore: strings(ignore),
        })
        .unwrap()
    };
    assert_eq!(
        select(None, &["magic-number"], &[]).len(),
        defaults.len() + 1
    );
    assert_eq!(select(Some(&["all"]), &[], &[]).len(), all_rules().len());
    assert_eq!(
        select(Some(&["upper-ell"]), &["need-braces"], &["upper-ell"]).len(),
        1
    );
    assert!(select(Some(&["all"]), &[], &["all"]).is_empty());
    assert!(RuleSet::all().without(&["all".to_string()]).is_empty());
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
    // What is left has no fix: the wildcard import, for one.
    assert!(
        outcome.remaining.iter().all(|d| d.fix.is_none()),
        "{outcome:?}"
    );
    assert!(
        outcome
            .remaining
            .iter()
            .any(|d| d.rule == "avoid-star-import")
    );
}

/// Lints `text` with one rule, after setting some of its options.
fn lines_with(rule: &str, options: &[(&str, RawValue)], text: &str) -> Vec<u32> {
    let mut rules = RuleSet::from_selection(Some(&[rule.to_string()]), &[]).unwrap();
    for (option, value) in options {
        rules.set_option(rule, option, value.clone()).unwrap();
    }
    let index = LineIndex::new(text);
    lint(text, &rules)
        .iter()
        .map(|d| index.line_col(text, d.range.start()).line)
        .collect()
}

#[test]
fn options_change_what_a_rule_reports() {
    let text = "class C {\n  int a = 3;\n  int b = 7;\n  int c = 0x10;\n}\n";
    assert_eq!(lines("magic-number", text), [2, 3, 4]);
    let ignore = RawValue::List(vec!["3".to_string(), "16".to_string()]);
    assert_eq!(
        lines_with("magic-number", &[("ignore-numbers", ignore)], text),
        [3]
    );
    assert!(
        lines_with(
            "magic-number",
            &[("ignore-field-declaration", RawValue::Bool(true))],
            text
        )
        .is_empty()
    );

    let text = "class C {\n  void m() {\n    int a;\n\n    // c\n  }\n}\n";
    assert!(lines("method-length", text).is_empty());
    assert_eq!(
        lines_with("method-length", &[("max", RawValue::Int(4))], text),
        [2]
    );
    // Without blank and comment lines, the method is its braces and one statement.
    let short = [
        ("max", RawValue::Int(3)),
        ("count-empty", RawValue::Bool(false)),
    ];
    assert!(lines_with("method-length", &short, text).is_empty());

    assert_eq!(
        lines_with("file-length", &[("max", RawValue::Int(6))], text),
        [1]
    );
    assert!(lines_with("file-length", &[("max", RawValue::Int(7))], text).is_empty());

    let text = "class C {\n  void m() { System.out.println(1); }  \n}\n";
    assert!(lines("regexp-singleline", text).is_empty());
    let trailing = [("format", RawValue::Str(r"\s+$".to_string()))];
    assert_eq!(lines_with("regexp-singleline", &trailing, text), [2]);

    let text = "class C {\n  void set_x(int x) {}\n  void setY(int y) {}\n}\n";
    assert_eq!(lines("method-name", text), [2]);
    let snake = [("format", RawValue::Str("^[a-z_]+$".to_string()))];
    assert_eq!(lines_with("method-name", &snake, text), [3]);
}

#[test]
fn options_are_checked() {
    let mut rules = RuleSet::all();
    let error =
        |result: Result<(), weavejig_linter::SelectionError>| result.unwrap_err().to_string();
    assert_eq!(
        error(rules.set_option("line-length", "maximum", RawValue::Int(1))),
        "rule `line-length` has no option `maximum`; its options are `max`, `ignore-pattern`, `tab-width`"
    );
    assert_eq!(
        error(rules.set_option("line-length", "max", RawValue::Bool(true))),
        "option `max` of rule `line-length` must be an integer"
    );
    assert_eq!(
        error(rules.set_option("upper-ell", "max", RawValue::Int(1))),
        "rule `upper-ell` has no option `max`: it has no options"
    );
    assert!(
        error(rules.set_option("method-name", "format", RawValue::Str("(".into())))
            .starts_with("option `format` of rule `method-name` is not a valid regular expression")
    );
    assert_eq!(
        error(rules.set_option("no-such-rule", "max", RawValue::Int(1))),
        "unknown lint rule `no-such-rule`"
    );
    // The options of a rule that is switched off are still checked.
    let mut none = RuleSet::from_selection(Some(&[]), &[]).unwrap();
    assert!(
        none.set_option("line-length", "max", RawValue::Int(1))
            .is_ok()
    );
    assert!(
        none.set_option("line-length", "maxx", RawValue::Int(1))
            .is_err()
    );
    // One string will do for a list.
    assert!(
        rules
            .set_option("illegal-import", "packages", RawValue::Str("sun".into()))
            .is_ok()
    );
}

#[test]
fn every_rule_documents_itself() {
    for rule in all_rules() {
        let meta = rule.meta();
        assert!(
            !meta.summary.is_empty() && !meta.summary.ends_with('.'),
            "{}",
            meta.id
        );
        for option in meta.options {
            assert!(
                option
                    .name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '-'),
                "{}: {}",
                meta.id,
                option.name
            );
            assert!(!option.help.is_empty(), "{}: {}", meta.id, option.name);
        }
    }
}

/// The lines `rule` reports in `text` when only `names` suppress.
fn unsuppressed(rule: &str, names: SuppressionNames, text: &str) -> Vec<u32> {
    let rules = RuleSet::from_selection(Some(&[rule.to_string()]), &[])
        .unwrap()
        .with_suppression_names(names);
    let index = LineIndex::new(text);
    lint(text, &rules)
        .iter()
        .map(|d| index.line_col(text, d.range.start()).line)
        .collect()
}

#[test]
fn suppressions() {
    let text = "\
class C {
  long a = 1l;
  long b = 2l; // weavejig-ignore: upper-ell
  // weavejig-ignore
  long c = 3l;
  long d = 4l; // weavejig-ignore: need-braces, other
  @SuppressWarnings(\"weavejig:upper-ell\")
  long e = 5l;
  @SuppressWarnings({\"unchecked\", \"weavejig\"})
  void m() {
    long f = 6l;
  }
  @SuppressWarnings(\"all\") long g = 7l;
  @SuppressWarnings(\"checkstyle:upperell\") long h = 8l;
  @SuppressWarnings(\"UpperEll\") long i = 9l;
  @SuppressWarnings(\"unchecked\") long j = 10l;
  // CHECKSTYLE:OFF
  long k = 11l;
  // CHECKSTYLE:ON
  long l = 12l;
  /* weavejig-ignore: upper-ell */
  long m = 13l;
  long n = 14l; // weavejig-ignored: not a marker
}
";
    assert_eq!(
        unsuppressed("upper-ell", SuppressionNames::default(), text),
        [2, 6, 16, 20, 23]
    );
    // Each tool's spellings can be switched off on their own.
    assert_eq!(
        unsuppressed("upper-ell", SuppressionNames::CHECKSTYLE, text),
        [2, 3, 5, 6, 8, 11, 16, 20, 22, 23]
    );
    assert_eq!(
        unsuppressed("upper-ell", SuppressionNames::NONE, text).len(),
        14
    );

    let file = "// weavejig-ignore-file: upper-ell\nclass C {\n  long a = 1l;\n  void m() { if (a > 0) a++; }\n}\n";
    assert!(unsuppressed("upper-ell", SuppressionNames::default(), file).is_empty());
    assert_eq!(
        unsuppressed("need-braces", SuppressionNames::default(), file),
        [4]
    );
    let everything = file.replace(": upper-ell", "");
    assert!(unsuppressed("need-braces", SuppressionNames::default(), &everything).is_empty());
}

#[test]
fn pmd_suppressions() {
    let text = "\
class C {
  void a(int n) { if (n > 0) n++; }
  void b(int n) { if (n > 0) n++; } // NOPMD
  @SuppressWarnings(\"PMD.ControlStatementBraces\")
  void c(int n) { if (n > 0) n++; }
  @SuppressWarnings(\"PMD\")
  void d(int n) { if (n > 0) n++; }
  @SuppressWarnings(\"PMD.SomethingElse\")
  void e(int n) { if (n > 0) n++; }
}
";
    assert_eq!(
        unsuppressed("need-braces", SuppressionNames::default(), text),
        [2, 9]
    );
    assert_eq!(
        unsuppressed("need-braces", SuppressionNames::PMD, text),
        [2, 9]
    );
    assert_eq!(
        unsuppressed("need-braces", SuppressionNames::CHECKSTYLE, text),
        [2, 3, 5, 7, 9]
    );
    // A rule that mirrors nothing of PMD's is not PMD's to switch off.
    let text = "class C {\n  long a = 1l; // NOPMD\n}\n";
    assert_eq!(
        unsuppressed("upper-ell", SuppressionNames::default(), text),
        [2]
    );
}

#[test]
fn rules_about_the_file_use_its_path() {
    let rules = RuleSet::from_selection(Some(&["outer-type-filename".to_string()]), &[]).unwrap();
    let text = "class Right {}\n";
    let at = |path: &str| lint(Source::new(text, Some(Path::new(path))), &rules).len();
    assert_eq!(at("src/Right.java"), 0);
    assert_eq!(at("src/Wrong.java"), 1);
    // Text that is not a file has no name to be wrong.
    assert!(lint(text, &rules).is_empty());
    // A later public type excuses the first.
    let text = "class Helper {}\npublic class Wrong {}\n";
    assert!(lint(Source::new(text, Some(Path::new("Wrong.java"))), &rules).is_empty());
}

#[test]
fn import_fixes_remove_whole_lines() {
    let text = "\
package p;

import java.lang.String;
import java.util.List;
import java.util.List;
import java.util.Map; import java.util.Set;
import p.Sibling;

class C {
  List<String> names;
  Set<Sibling> siblings;
}
";
    assert_eq!(lines("redundant-import", text), [3, 5, 7]);
    assert_eq!(lines("unused-imports", text), [3, 6]);
    let rules = RuleSet::from_selection(
        Some(&["redundant-import".to_string(), "unused-imports".to_string()]),
        &[],
    )
    .unwrap();
    assert_eq!(
        fix_all(text, &rules).text,
        "package p;\n\nimport java.util.List;\n import java.util.Set;\n\nclass C {\n  List<String> names;\n  Set<Sibling> siblings;\n}\n"
    );
}

#[test]
fn modifier_fixes() {
    let text = "\
interface I {
  public static final int A = 1;
  public abstract void m(final int x);
}
final class C {
  private final void p() {}
  int zero = 0, one = 1;
  String none = null;
  void q(int a, @Nullable String b, final int c) {
    try (final Reader r = open()) {}
  }
}
";
    assert_eq!(
        fixed("redundant-modifier", text),
        "\
interface I {
  int A = 1;
  void m(int x);
}
final class C {
  private void p() {}
  int zero = 0, one = 1;
  String none = null;
  void q(int a, @Nullable String b, final int c) {
    try (Reader r = open()) {}
  }
}
"
    );
    let initialized = fixed("explicit-initialization", text);
    assert!(initialized.contains("int zero, one = 1;"), "{initialized}");
    assert!(initialized.contains("String none;"), "{initialized}");
    let finals = fixed("final-parameters", text);
    assert!(
        finals.contains("void q(final int a, @Nullable final String b, final int c)"),
        "{finals}"
    );
    // Parameters of a method without a body are not asked to be final.
    assert!(finals.contains("void m(final int x);"), "{finals}");
    assert!(lines("final-parameters", &finals).is_empty());
}

#[test]
fn text_fixes() {
    assert_eq!(
        fixed("newline-at-end-of-file", "class C {}"),
        "class C {}\n"
    );
    assert_eq!(
        fixed("newline-at-end-of-file", "class C {\r\n}"),
        "class C {\r\n}\r\n"
    );
    assert_eq!(
        fixed("newline-at-end-of-file", "class C {}\n"),
        "class C {}\n"
    );
    assert!(lines("newline-at-end-of-file", "").is_empty());
    // After the constants of an enum, a second `;` is an empty statement.
    assert_eq!(
        fixed("empty-statement", "enum E {\n  A;\n  ;\n}\n"),
        "enum E {\n  A;\n  \n}\n"
    );
}
