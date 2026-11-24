//! The lint probes in `corpus/smoke/lint`: what every rule reports for them
//! is recorded in `diagnostics.txt`, which the scoring harness has checked
//! against Checkstyle and PMD. See the README there.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use weavejig_linter::{RuleSet, Source, all_rules, lint};
use weavejig_syntax::LineIndex;

fn probes_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/smoke/lint")
}

/// Rules the probes cannot trip with default options: a file would have to
/// be thousands of lines long, or the rule does nothing until configured.
/// They are tested in `rules.rs`.
const NOT_PROBED: [&str; 3] = ["file-length", "method-length", "regexp-singleline"];

#[test]
fn probes_report_what_is_recorded() {
    let dir = probes_dir();
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "java"))
        .collect();
    files.sort();
    assert!(!files.is_empty());

    let rules = RuleSet::all();
    let mut actual = String::new();
    let mut reported = BTreeSet::new();
    for path in &files {
        let text = fs::read_to_string(path).unwrap();
        let index = LineIndex::new(&text);
        let name = path.file_name().unwrap().to_string_lossy();
        for diagnostic in lint(Source::new(&text, Some(path)), &rules) {
            let at = index.line_col(&text, diagnostic.range.start());
            actual.push_str(&format!(
                "{name}:{}:{}: {}\n",
                at.line, at.column, diagnostic.rule
            ));
            reported.insert(diagnostic.rule);
        }
    }

    let recorded = dir.join("diagnostics.txt");
    if std::env::var_os("WEAVEJIG_UPDATE_FIXTURES").is_some() {
        fs::write(&recorded, &actual).unwrap();
    }
    let expected = fs::read_to_string(&recorded).unwrap_or_default();
    assert!(
        actual == expected,
        "the lint probes report something else than `diagnostics.txt` records;\n\
         see corpus/smoke/lint/README.md\n{}",
        first_difference(&expected, &actual)
    );

    // A rule the probes never trip is a rule nothing would notice breaking.
    for rule in all_rules() {
        let id = rule.meta().id;
        assert!(
            reported.contains(id) || NOT_PROBED.contains(&id),
            "no probe in corpus/smoke/lint trips `{id}`"
        );
    }
}

fn first_difference(expected: &str, actual: &str) -> String {
    let (mut expected, mut actual) = (expected.lines(), actual.lines());
    loop {
        match (expected.next(), actual.next()) {
            (Some(e), Some(a)) if e == a => {}
            (None, None) => return String::new(),
            (e, a) => {
                return format!(
                    "first difference:\n  recorded: {}\n  reported: {}",
                    e.unwrap_or("(nothing more)"),
                    a.unwrap_or("(nothing more)")
                );
            }
        }
    }
}
