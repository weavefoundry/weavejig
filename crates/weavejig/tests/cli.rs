//! End-to-end tests of the `weavejig` binary.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const UNTIDY: &str = "class A{long x=1l;void m(){if(x>0)x++;}}\n";
const TIDY: &str =
    "class A {\n  long x = 1L;\n\n  void m() {\n    if (x > 0) {\n      x++;\n    }\n  }\n}\n";

/// A fresh directory for one test, holding the given files.
fn project(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    for (path, text) in files {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn weavejig(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_weavejig"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

fn weavejig_stdin(dir: &Path, args: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_weavejig"))
        .current_dir(dir)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[test]
fn check_reports_problems_and_exits_1() {
    let dir = project(
        "check_reports",
        &[("src/A.java", UNTIDY), ("src/Ok.java", "class Ok {}\n")],
    );
    let output = weavejig(&dir, &["check"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("src/A.java:1:16: upper-ell: "), "{text}");
    assert!(text.contains("src/A.java:1:28: need-braces: "), "{text}");
    assert!(
        text.contains("Found 2 problems in 1 file (2 fixable with `--fix`)."),
        "{text}"
    );
}

#[test]
fn check_clean_project_exits_0() {
    let dir = project("check_clean", &[("A.java", TIDY)]);
    let output = weavejig(&dir, &["check", "."]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "No problems in 1 file.\n");
}

#[test]
fn check_json_has_exact_locations_and_fixes() {
    let dir = project("check_json", &[("A.java", UNTIDY)]);
    let output = weavejig(
        &dir,
        &["check", "--output-format", "json", "--select", "upper-ell"],
    );
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(report["version"], 1);
    assert_eq!(report["summary"]["files_checked"], 1);
    assert_eq!(report["summary"]["diagnostics"], 1);
    assert_eq!(report["summary"]["fixable"], 1);
    let diagnostic = &report["diagnostics"][0];
    assert_eq!(diagnostic["file"], "A.java");
    assert_eq!(diagnostic["rule"], "upper-ell");
    assert_eq!(diagnostic["severity"], "warning");
    assert_eq!(
        diagnostic["range"]["start"],
        serde_json::json!({"line": 1, "column": 16, "offset": 15})
    );
    assert_eq!(
        diagnostic["range"]["end"],
        serde_json::json!({"line": 1, "column": 18, "offset": 17})
    );
    // Applying the edit by byte offset fixes the problem.
    let edit = &diagnostic["fix"]["edits"][0];
    let (start, end) = (
        edit["range"]["start"]["offset"].as_u64().unwrap() as usize,
        edit["range"]["end"]["offset"].as_u64().unwrap() as usize,
    );
    let mut fixed = UNTIDY.to_string();
    fixed.replace_range(start..end, edit["replacement"].as_str().unwrap());
    assert!(fixed.contains("x=1L;"), "{fixed}");
}

#[test]
fn check_fix_rewrites_files() {
    let dir = project("check_fix", &[("A.java", UNTIDY)]);
    let output = weavejig(&dir, &["check", "--fix"]);
    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));
    assert!(stdout(&output).contains("Fixed 2 problems."));
    let fixed = fs::read_to_string(dir.join("A.java")).unwrap();
    assert_eq!(fixed, "class A{long x=1L;void m(){if(x>0){ x++; }}}\n");
}

#[test]
fn syntax_errors_are_diagnostics() {
    let dir = project(
        "syntax_errors",
        &[("Bad.java", "class Bad {\n  int x = ;\n}\n")],
    );
    let output = weavejig(&dir, &["check", "--output-format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(report["diagnostics"][0]["rule"], "syntax-error");
    assert_eq!(report["diagnostics"][0]["severity"], "error");
    assert_eq!(report["diagnostics"][0]["range"]["start"]["line"], 2);

    // The formatter refuses the file, leaves it alone, and says why.
    let output = weavejig(&dir, &["format"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("Bad.java:2:11: syntax-error: expected an expression"));
    assert_eq!(
        fs::read_to_string(dir.join("Bad.java")).unwrap(),
        "class Bad {\n  int x = ;\n}\n"
    );
}

#[test]
fn format_rewrites_files_and_check_mode_does_not() {
    let untidy = "class A{int x;}\n";
    let dir = project("format", &[("A.java", untidy), ("B.java", "class B {}\n")]);

    let output = weavejig(&dir, &["format", "--check"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stdout(&output),
        "Would reformat: A.java\n1 file would be reformatted, 1 unchanged.\n"
    );
    assert_eq!(fs::read_to_string(dir.join("A.java")).unwrap(), untidy);

    let output = weavejig(&dir, &["format"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "1 file reformatted, 1 unchanged.\n");
    assert_eq!(
        fs::read_to_string(dir.join("A.java")).unwrap(),
        "class A {\n  int x;\n}\n"
    );

    assert_eq!(
        weavejig(&dir, &["format", "--check"]).status.code(),
        Some(0)
    );
}

#[test]
fn an_unreadable_file_does_not_stop_the_others() {
    let dir = project(
        "unreadable",
        &[("A.java", "class A{int x;}\n"), ("C.java", UNTIDY)],
    );
    // Latin-1, not UTF-8.
    fs::write(dir.join("B.java"), b"class B { // caf\xe9\n}\n").unwrap();

    let output = weavejig(&dir, &["format"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("cannot read B.java"),
        "{}",
        stderr(&output)
    );
    assert_eq!(
        stdout(&output),
        "2 files reformatted, 0 unchanged, 1 could not be formatted.\n"
    );

    let output = weavejig(&dir, &["check"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("cannot read B.java"),
        "{}",
        stderr(&output)
    );
    assert!(stdout(&output).contains("C.java:"), "{}", stdout(&output));
}

#[test]
fn stdin() {
    let dir = project("stdin", &[]);
    let output = weavejig_stdin(&dir, &["format", "-"], "class A{int x;}");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "class A {\n  int x;\n}\n");

    let output = weavejig_stdin(&dir, &["format", "--style", "aosp", "-"], "class A{int x;}");
    assert_eq!(stdout(&output), "class A {\n    int x;\n}\n");

    let output = weavejig_stdin(&dir, &["check", "-"], UNTIDY);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).contains("<stdin>:1:16: upper-ell"));

    // With --fix, the fixed source is the output.
    let output = weavejig_stdin(
        &dir,
        &["check", "--fix", "--select", "upper-ell", "-"],
        UNTIDY,
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), UNTIDY.replace("1l", "1L"));
}

#[test]
fn configuration_file() {
    let config = "exclude = [\"generated/**\"]\n\n[format]\nstyle = \"aosp\"\n\n[lint]\nignore = [\"need-braces\"]\n";
    let dir = project(
        "configuration",
        &[
            ("weavejig.toml", config),
            ("src/A.java", UNTIDY),
            ("generated/G.java", UNTIDY),
        ],
    );
    // Found from a subdirectory, too.
    let output = weavejig(&dir.join("src"), &["check"]);
    let text = stdout(&output);
    assert!(text.contains("upper-ell"), "{text}");
    assert!(!text.contains("need-braces"), "{text}");

    let output = weavejig(&dir, &["check"]);
    assert!(
        !stdout(&output).contains("generated"),
        "{}",
        stdout(&output)
    );
    assert!(
        stdout(&output).contains("Found 1 problem in 1 file"),
        "{}",
        stdout(&output)
    );

    // `exclude` is relative to the configuration file, wherever we run from.
    let output = weavejig(&dir.join("generated"), &["check"]);
    assert_eq!(stdout(&output), "No problems in 0 files.\n");
    let output = weavejig(&dir.join("src"), &["check", ".."]);
    assert!(
        !stdout(&output).contains("generated"),
        "{}",
        stdout(&output)
    );

    weavejig(&dir, &["format"]);
    assert!(
        fs::read_to_string(dir.join("src/A.java"))
            .unwrap()
            .contains("\n    long x = 1l;\n")
    );
    assert_eq!(
        fs::read_to_string(dir.join("generated/G.java")).unwrap(),
        UNTIDY
    );
}

#[test]
fn mistakes_exit_2_with_a_message() {
    let dir = project("mistakes", &[("A.java", TIDY)]);
    let output = weavejig(&dir, &["check", "--select", "no-such-rule"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("unknown lint rule `no-such-rule`"),
        "{}",
        stderr(&output)
    );

    let output = weavejig(&dir, &["check", "missing-dir"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("no such file or directory"),
        "{}",
        stderr(&output)
    );

    fs::write(dir.join("weavejig.toml"), "[lint]\nselcet = []\n").unwrap();
    let output = weavejig(&dir, &["check"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("invalid configuration file"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn rules_lists_every_rule() {
    let dir = project("rules", &[]);
    let output = weavejig(&dir, &["rules", "--output-format", "json"]);
    let rules: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    let ids: Vec<&str> = rules
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert!(
        ids.contains(&"upper-ell") && ids.contains(&"need-braces"),
        "{ids:?}"
    );
    assert!(rules[0]["checkstyle"].is_string());
}
