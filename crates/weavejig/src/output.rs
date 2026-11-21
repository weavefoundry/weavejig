//! The JSON shape of diagnostics, shared by `--output-format json` and the
//! text renderer.
//!
//! This is a public interface for editors, CI and coding agents: change it
//! only by adding fields, and bump [`FORMAT_VERSION`] if a change could break
//! a consumer.

use serde::Serialize;
use weavejig_linter::{Diagnostic, SYNTAX_ERROR};
use weavejig_syntax::{LineIndex, TextRange, TextSize};

pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Serialize)]
pub struct Report {
    pub version: u32,
    pub diagnostics: Vec<JsonDiagnostic>,
    pub summary: Summary,
}

#[derive(Debug, Default, Serialize)]
pub struct Summary {
    pub files_checked: usize,
    pub files_with_diagnostics: usize,
    pub diagnostics: usize,
    /// How many of `diagnostics` carry a fix.
    pub fixable: usize,
    /// Fixes applied by this run (`--fix`); their diagnostics are not listed.
    pub fixed: usize,
}

#[derive(Debug, Serialize)]
pub struct JsonDiagnostic {
    pub file: String,
    pub rule: &'static str,
    pub severity: Severity,
    pub message: String,
    pub range: JsonRange,
    pub fix: Option<JsonFix>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// A half-open range: `end` is the position just past the last character.
#[derive(Debug, Serialize)]
pub struct JsonRange {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Serialize)]
pub struct Position {
    /// 1-based line number.
    pub line: u32,
    /// 1-based column, counted in Unicode code points.
    pub column: u32,
    /// 0-based byte offset into the UTF-8 file.
    pub offset: u32,
}

#[derive(Debug, Serialize)]
pub struct JsonFix {
    pub description: String,
    /// Non-overlapping replacements. Apply them all, or none.
    pub edits: Vec<JsonEdit>,
}

#[derive(Debug, Serialize)]
pub struct JsonEdit {
    pub range: JsonRange,
    pub replacement: String,
}

fn position(text: &str, index: &LineIndex, offset: TextSize) -> Position {
    let line_col = index.line_col(text, offset);
    Position {
        line: line_col.line,
        column: line_col.column,
        offset: offset.into(),
    }
}

fn range(text: &str, index: &LineIndex, range: TextRange) -> JsonRange {
    JsonRange {
        start: position(text, index, range.start()),
        end: position(text, index, range.end()),
    }
}

/// Converts the diagnostics of one file, whose contents are `text`.
pub fn convert(file: &str, text: &str, diagnostics: &[Diagnostic]) -> Vec<JsonDiagnostic> {
    let index = LineIndex::new(text);
    diagnostics
        .iter()
        .map(|diagnostic| JsonDiagnostic {
            file: file.to_string(),
            rule: diagnostic.rule,
            severity: if diagnostic.rule == SYNTAX_ERROR {
                Severity::Error
            } else {
                Severity::Warning
            },
            message: diagnostic.message.clone(),
            range: range(text, &index, diagnostic.range),
            fix: diagnostic.fix.as_ref().map(|fix| JsonFix {
                description: fix.description.clone(),
                edits: fix
                    .edits
                    .iter()
                    .map(|edit| JsonEdit {
                        range: range(text, &index, edit.range),
                        replacement: edit.replacement.clone(),
                    })
                    .collect(),
            }),
        })
        .collect()
}

/// One line per diagnostic: `file:line:column: rule: message`.
pub fn render_text(report: &Report) -> String {
    let mut out = String::new();
    for diagnostic in &report.diagnostics {
        let start = &diagnostic.range.start;
        out.push_str(&format!(
            "{}:{}:{}: {}: {}{}\n",
            diagnostic.file,
            start.line,
            start.column,
            diagnostic.rule,
            diagnostic.message,
            if diagnostic.fix.is_some() {
                " [fixable]"
            } else {
                ""
            },
        ));
    }
    let summary = &report.summary;
    if summary.fixed > 0 {
        out.push_str(&format!("Fixed {}.\n", count(summary.fixed, "problem")));
    }
    if summary.diagnostics == 0 {
        out.push_str(&format!(
            "No problems in {}.\n",
            count(summary.files_checked, "file")
        ));
    } else {
        out.push_str(&format!(
            "Found {} in {}",
            count(summary.diagnostics, "problem"),
            count(summary.files_with_diagnostics, "file")
        ));
        if summary.fixable > 0 {
            out.push_str(&format!(" ({} fixable with `--fix`)", summary.fixable));
        }
        out.push_str(".\n");
    }
    out
}

pub fn count(n: usize, noun: &str) -> String {
    format!("{n} {noun}{}", if n == 1 { "" } else { "s" })
}
