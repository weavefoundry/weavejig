//! Fixture tests: `NAME.input.java` must format to `NAME.expected.java`.
//! See `fixtures/README.md`.
//!
//! The fixtures in `fixtures/` record WeaveJig's own output. Those in
//! `fixtures/reference/` record google-java-format's output and are never
//! regenerated from WeaveJig.

use std::fs;
use std::path::Path;

use weavejig_formatter::{FormatOptions, format};

#[test]
fn fixtures() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let update = std::env::var_os("WEAVEJIG_UPDATE_FIXTURES").is_some();
    let mut checked = 0;
    let mut failures = Vec::new();
    for entry in fs::read_dir(&dir).unwrap() {
        let input_path = entry.unwrap().path();
        let Some(name) = input_path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".input.java"))
        else {
            continue;
        };
        let expected_path = dir.join(format!("{name}.expected.java"));
        let input = fs::read_to_string(&input_path).unwrap();
        let actual = format(&input, &FormatOptions::default())
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        if update {
            fs::write(&expected_path, &actual).unwrap();
        }
        let expected = fs::read_to_string(&expected_path).unwrap_or_default();
        if actual != expected {
            failures.push(format!(
                "--- {name}: expected\n{expected}\n--- {name}: actual\n{actual}"
            ));
        }
        // Formatting its own output must change nothing.
        let again = format(&actual, &FormatOptions::default()).unwrap();
        assert_eq!(again, actual, "{name}: formatting is not idempotent");
        checked += 1;
    }
    assert!(checked > 0, "no fixtures found in {}", dir.display());
    assert!(
        failures.is_empty(),
        "{}\n\nIf the change is intended, rerun with WEAVEJIG_UPDATE_FIXTURES=1.",
        failures.join("\n")
    );
}

#[test]
fn reference_fixtures() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reference");
    let mut checked = 0;
    let mut failures = Vec::new();
    for entry in fs::read_dir(&dir).unwrap() {
        let input_path = entry.unwrap().path();
        let Some(name) = input_path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".input.java"))
        else {
            continue;
        };
        let input = fs::read_to_string(&input_path).unwrap();
        let expected = fs::read_to_string(dir.join(format!("{name}.expected.java"))).unwrap();
        let actual = format(&input, &FormatOptions::default())
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        if actual != expected {
            failures.push(format!(
                "--- {name}: google-java-format\n{expected}\n--- {name}: WeaveJig\n{actual}"
            ));
        }
        checked += 1;
    }
    assert!(checked > 0, "no fixtures found in {}", dir.display());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
