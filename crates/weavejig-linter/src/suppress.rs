//! Suppressions: the ways source code switches diagnostics off.
//!
//! - `@SuppressWarnings("weavejig:rule-id")` on a declaration covers the
//!   declaration. `"weavejig"` and `"all"` cover every rule.
//! - `// weavejig-ignore: rule-id, other-rule` after code covers its line; on
//!   a line of its own, the next line. Without rule ids it covers every rule.
//! - `// weavejig-ignore-file: rule-id` covers the whole file.
//!
//! A `weavejig-ignore` comment must start with those words; one that only
//! mentions them is an ordinary comment.
//!
//! So that code written for Checkstyle or PMD needs no changes, their own
//! spellings work too, for the rules that mirror their checks:
//!
//! - `@SuppressWarnings("checkstyle:magicnumber")` and
//!   `@SuppressWarnings("PMD.UnusedPrivateField")` (`"PMD"` for all of PMD's);
//! - `// CHECKSTYLE:OFF` up to `// CHECKSTYLE:ON`;
//! - `// NOPMD` on the offending line.

use weavejig_syntax::{
    LineIndex, SyntaxKind::*, SyntaxNode, SyntaxToken, TextRange, TextSize, child_node,
};

use crate::{RuleMeta, all_rules};

/// Which spellings of a suppression count.
///
/// Everything is on by default. The scoring harness narrows this to the
/// spellings the reference tool itself honours, so that a suppression meant
/// for one tool does not hide a diagnostic the other reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuppressionNames {
    /// `weavejig:` names and `weavejig-ignore` comments.
    pub weavejig: bool,
    /// `checkstyle:` names and `CHECKSTYLE:OFF` comments.
    pub checkstyle: bool,
    /// `PMD.` names and `NOPMD` comments.
    pub pmd: bool,
}

impl Default for SuppressionNames {
    fn default() -> SuppressionNames {
        SuppressionNames {
            weavejig: true,
            checkstyle: true,
            pmd: true,
        }
    }
}

impl SuppressionNames {
    pub const NONE: SuppressionNames = SuppressionNames {
        weavejig: false,
        checkstyle: false,
        pmd: false,
    };
    pub const CHECKSTYLE: SuppressionNames = SuppressionNames {
        weavejig: false,
        checkstyle: true,
        pmd: false,
    };
    pub const PMD: SuppressionNames = SuppressionNames {
        weavejig: false,
        checkstyle: false,
        pmd: true,
    };
}

/// What a suppression switches off.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Scope {
    All,
    /// Every rule that mirrors a Checkstyle check.
    Checkstyle,
    /// Every rule that mirrors a PMD rule.
    Pmd,
    Rules(Vec<&'static str>),
}

impl Scope {
    fn covers(&self, meta: &RuleMeta) -> bool {
        match self {
            Scope::All => true,
            Scope::Checkstyle => meta.checkstyle.is_some(),
            Scope::Pmd => meta.pmd.is_some(),
            Scope::Rules(ids) => ids.contains(&meta.id),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Suppressions {
    file: Vec<Scope>,
    /// By the text they cover.
    regions: Vec<(TextRange, Scope)>,
    /// By the 1-based line they cover.
    lines: Vec<(u32, Scope)>,
}

impl Suppressions {
    pub(crate) fn is_empty(&self) -> bool {
        self.file.is_empty() && self.regions.is_empty() && self.lines.is_empty()
    }

    /// Whether a diagnostic of rule `meta` that starts at `offset`, on
    /// `line`, is suppressed.
    pub(crate) fn covers(&self, meta: &RuleMeta, offset: TextSize, line: u32) -> bool {
        self.file.iter().any(|scope| scope.covers(meta))
            || self
                .lines
                .iter()
                .any(|(at, scope)| *at == line && scope.covers(meta))
            || self
                .regions
                .iter()
                .any(|(range, scope)| range.contains_inclusive(offset) && scope.covers(meta))
    }

    pub(crate) fn collect(
        root: &SyntaxNode,
        lines: &LineIndex,
        text: &str,
        names: SuppressionNames,
    ) -> Suppressions {
        let mut found = Suppressions::default();
        if names == SuppressionNames::NONE {
            return found;
        }
        // Skip the tree walk for the many files that suppress nothing.
        let mentions = |word: &str| text.contains(word);
        let annotations = mentions("SuppressWarnings");
        let comments = (names.weavejig && mentions("weavejig-ignore"))
            || (names.checkstyle && mentions("CHECKSTYLE:OFF"))
            || (names.pmd && mentions("NOPMD"));
        if !annotations && !comments {
            return found;
        }

        // The `CHECKSTYLE:OFF` comment a region is open from.
        let mut off: Option<TextSize> = None;
        let mut token = root.first_token();
        while let Some(current) = token {
            match current.kind() {
                IDENT if annotations && current.text() == "SuppressWarnings" => {
                    found.annotation(&current, names);
                }
                LINE_COMMENT | BLOCK_COMMENT if comments => {
                    found.comment(&current, lines, text, names, &mut off);
                }
                _ => {}
            }
            token = current.next_token();
        }
        if let Some(start) = off {
            let end = TextSize::try_from(text.len()).unwrap_or_default();
            found
                .regions
                .push((TextRange::new(start, end), Scope::Checkstyle));
        }
        found
    }

    /// `name` is the `SuppressWarnings` of what may be an annotation.
    fn annotation(&mut self, name: &SyntaxToken, names: SuppressionNames) {
        let Some(annotation) = name
            .parent()
            .filter(|parent| parent.kind() == NAME)
            .and_then(|parent| parent.parent())
            .filter(|parent| parent.kind() == ANNOTATION)
        else {
            return;
        };
        let Some(args) = child_node(&annotation, ANNOTATION_ARGS) else {
            return;
        };
        // The declaration, parameter or variable the annotation is on.
        let Some(target) = annotation
            .parent()
            .filter(|modifiers| modifiers.kind() == MODIFIERS)
            .and_then(|modifiers| modifiers.parent())
        else {
            return;
        };
        let mut rules = Vec::new();
        for literal in args
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|token| token.kind() == STRING_LITERAL)
        {
            let value = literal.text().trim_matches('"');
            match scope_of(value, names) {
                Some(Scope::Rules(ids)) => rules.extend(ids),
                Some(scope) => self.regions.push((target.text_range(), scope)),
                None => {}
            }
        }
        if !rules.is_empty() {
            self.regions
                .push((target.text_range(), Scope::Rules(rules)));
        }
    }

    fn comment(
        &mut self,
        comment: &SyntaxToken,
        lines: &LineIndex,
        text: &str,
        names: SuppressionNames,
        off: &mut Option<TextSize>,
    ) {
        let body = comment.text();
        let start = comment.text_range().start();
        if names.checkstyle {
            if body.contains("CHECKSTYLE:OFF") {
                off.get_or_insert(start);
            } else if body.contains("CHECKSTYLE:ON")
                && let Some(from) = off.take()
            {
                self.regions
                    .push((TextRange::new(from, start), Scope::Checkstyle));
            }
        }
        let line = lines.line_col(text, start).line;
        if names.pmd && comment.kind() == LINE_COMMENT && body.contains("NOPMD") {
            self.lines.push((line, Scope::Pmd));
        }
        if !names.weavejig {
            return;
        }
        if let Some(rest) = after(body, "weavejig-ignore-file") {
            self.file.push(listed(rest));
        } else if let Some(rest) = after(body, "weavejig-ignore") {
            let end_line = lines.line_col(text, comment.text_range().end()).line;
            // After code, the comment is about its own line; alone on a
            // line, about the next one.
            let trails_code = text
                [usize::from(lines.line_range(text, line).start())..usize::from(start)]
                .chars()
                .any(|c| !c.is_whitespace());
            let target = if trails_code { line } else { end_line + 1 };
            self.lines.push((target, listed(rest)));
        }
    }
}

/// What follows `marker` in a comment that starts with it. A comment that
/// only mentions the marker further on, as documentation might, is not a
/// suppression.
fn after<'a>(comment: &'a str, marker: &str) -> Option<&'a str> {
    let text = comment
        .strip_prefix("//")
        .or_else(|| comment.strip_prefix("/*"))
        .unwrap_or(comment);
    let rest = text.trim_start().strip_prefix(marker)?;
    // `weavejig-ignore` must not match `weavejig-ignore-file`, nor a longer
    // word.
    if rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    Some(rest)
}

/// The rules listed after a marker: `: a, b`. Nothing listed means all.
fn listed(rest: &str) -> Scope {
    let Some(list) = rest.trim_start().strip_prefix(':') else {
        return Scope::All;
    };
    let ids: Vec<&'static str> = list
        .trim_end_matches("*/")
        .split(',')
        .filter_map(|item| item.split_whitespace().next())
        .filter_map(|id| {
            all_rules()
                .iter()
                .map(|rule| rule.meta().id)
                .find(|known| *known == id)
        })
        .collect();
    Scope::Rules(ids)
}

/// What a `@SuppressWarnings` value switches off, if anything of ours.
fn scope_of(value: &str, names: SuppressionNames) -> Option<Scope> {
    if value == "all" {
        return Some(Scope::All);
    }
    let rule_where = |matches: &dyn Fn(&RuleMeta) -> bool| {
        let ids: Vec<&'static str> = all_rules()
            .iter()
            .map(|rule| rule.meta())
            .filter(|meta| matches(meta))
            .map(|meta| meta.id)
            .collect();
        (!ids.is_empty()).then_some(Scope::Rules(ids))
    };
    if names.weavejig {
        if value == "weavejig" || value == "weavejig:all" {
            return Some(Scope::All);
        }
        if let Some(id) = value.strip_prefix("weavejig:") {
            return rule_where(&|meta| meta.id == id);
        }
    }
    if names.pmd {
        if value == "PMD" {
            return Some(Scope::Pmd);
        }
        if let Some(name) = value.strip_prefix("PMD.") {
            return rule_where(&|meta| meta.pmd.is_some_and(|rule| rule.name == name));
        }
    }
    if names.checkstyle {
        // Checkstyle matches the check's name without regard to case, with
        // or without the `checkstyle:` prefix.
        let name = value.to_ascii_lowercase();
        let name = name.strip_prefix("checkstyle:").unwrap_or(&name);
        let name = name.strip_suffix("check").unwrap_or(name);
        return rule_where(&|meta| {
            meta.checkstyle
                .is_some_and(|check| check.eq_ignore_ascii_case(name))
        });
    }
    None
}
