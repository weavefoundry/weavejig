//! Applying fixes to source text.

use weavejig_syntax::TextSize;

use crate::{Diagnostic, RuleSet, Source, lint};

/// Fixes can enable or conflict with other fixes, so they are applied in
/// passes. Real code settles in two or three; the cap guards against a pair
/// of rules that undo each other.
const MAX_PASSES: usize = 10;

/// Applies the fixes of `diagnostics` to `text`.
///
/// Fixes are taken in source order. A fix whose edits overlap or touch a fix
/// already taken is skipped, so the result is always well-formed; the skipped
/// ones can be picked up by linting again. Returns the new text and the
/// number of fixes applied.
pub fn apply_fixes(text: &str, diagnostics: &[Diagnostic]) -> (String, usize) {
    let mut fixes: Vec<_> = diagnostics.iter().filter_map(|d| d.fix.as_ref()).collect();
    fixes.retain(|fix| !fix.edits.is_empty());
    fixes.sort_by_key(|fix| fix.edits.iter().map(|e| e.range.start()).min());

    let mut taken = Vec::new();
    let mut applied = 0;
    for fix in fixes {
        let conflicts = fix.edits.iter().enumerate().any(|(i, edit)| {
            taken
                .iter()
                .chain(&fix.edits[..i])
                .any(|other: &crate::Edit| {
                    // Touching counts as a conflict: two insertions at one offset
                    // have no defined order.
                    edit.range.start() <= other.range.end()
                        && other.range.start() <= edit.range.end()
                })
        });
        if !conflicts {
            taken.extend(fix.edits.iter().cloned());
            applied += 1;
        }
    }

    taken.sort_by_key(|edit| edit.range.start());
    let mut out = String::with_capacity(text.len());
    let mut last = TextSize::new(0);
    for edit in &taken {
        out.push_str(&text[usize::from(last)..usize::from(edit.range.start())]);
        out.push_str(&edit.replacement);
        last = edit.range.end();
    }
    out.push_str(&text[usize::from(last)..]);
    (out, applied)
}

#[derive(Debug, Clone)]
pub struct FixOutcome {
    /// The source text after fixing.
    pub text: String,
    /// How many fixes were applied, over all passes.
    pub fixed: usize,
    /// Diagnostics that remain in the fixed text.
    pub remaining: Vec<Diagnostic>,
}

/// Lints `text` and applies fixes until none are left to apply.
pub fn fix_all<'a>(source: impl Into<Source<'a>>, rules: &RuleSet) -> FixOutcome {
    let source = source.into();
    let path = source.path;
    let mut text = source.text.to_string();
    let mut fixed = 0;
    for _ in 0..MAX_PASSES {
        let diagnostics = lint(Source::new(&text, path), rules);
        let (next, applied) = apply_fixes(&text, &diagnostics);
        if applied == 0 {
            return FixOutcome {
                text,
                fixed,
                remaining: diagnostics,
            };
        }
        // A fix must never break the code. If one does, keep the last text
        // that parsed and report what is left.
        if !weavejig_syntax::parse(&next).is_ok() {
            debug_assert!(false, "a lint fix produced code that does not parse");
            return FixOutcome {
                text,
                fixed,
                remaining: diagnostics,
            };
        }
        text = next;
        fixed += applied;
    }
    let remaining = lint(Source::new(&text, path), rules);
    FixOutcome {
        text,
        fixed,
        remaining,
    }
}
