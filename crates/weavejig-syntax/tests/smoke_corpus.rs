//! Parses every file in the smoke corpus.

use std::fs;
use std::path::{Path, PathBuf};

use weavejig_syntax::{SyntaxKind, parse};

fn java_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            java_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "java") {
            out.push(path);
        }
    }
}

fn smoke_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/smoke");
    let mut files = Vec::new();
    java_files(&root, &mut files);
    files.sort();
    assert!(files.len() >= 8, "smoke corpus is missing");
    files
}

#[test]
fn trees_are_lossless() {
    for path in smoke_files() {
        let text = fs::read_to_string(&path).unwrap();
        let parse = parse(&text);
        assert_eq!(
            parse.syntax().text().to_string(),
            text,
            "{}",
            path.display()
        );
        assert_eq!(parse.syntax().kind(), SyntaxKind::COMPILATION_UNIT);
    }
}

#[test]
fn valid_files_parse_without_errors() {
    for path in smoke_files() {
        let invalid = path.components().any(|c| c.as_os_str() == "invalid");
        let text = fs::read_to_string(&path).unwrap();
        let parse = parse(&text);
        if invalid {
            assert!(!parse.is_ok(), "{} should be rejected", path.display());
        } else {
            assert!(parse.is_ok(), "{}: {:#?}", path.display(), parse.errors());
            let has_error_node = parse
                .syntax()
                .descendants()
                .any(|node| node.kind() == SyntaxKind::ERROR);
            assert!(!has_error_node, "{}", path.display());
        }
    }
}
