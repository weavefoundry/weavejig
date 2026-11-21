//! Syntax-level lint rules for Java, with automatic fixes.
//!
//! Rules are reimplemented from the documented and observed behaviour of
//! Checkstyle and PMD. Each rule records which reference check it mirrors so
//! the scoring harness can compare diagnostics rule by rule.

mod fix;
mod rules;

use weavejig_syntax::{NodeOrToken, SyntaxNode, SyntaxToken, TextRange, WalkEvent, parse};

pub use crate::fix::{FixOutcome, apply_fixes, fix_all};

/// The rule id used for lexical and syntax errors.
pub const SYNTAX_ERROR: &str = "syntax-error";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Id of the rule that produced this diagnostic, or [`SYNTAX_ERROR`].
    pub rule: &'static str,
    pub message: String,
    pub range: TextRange,
    pub fix: Option<Fix>,
}

/// A set of edits that together resolve one diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fix {
    pub description: String,
    pub edits: Vec<Edit>,
}

/// Replaces the text in `range` with `replacement`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub range: TextRange,
    pub replacement: String,
}

impl Edit {
    pub fn replace(range: TextRange, replacement: impl Into<String>) -> Edit {
        Edit {
            range,
            replacement: replacement.into(),
        }
    }

    pub fn insert(offset: weavejig_syntax::TextSize, text: impl Into<String>) -> Edit {
        Edit {
            range: TextRange::empty(offset),
            replacement: text.into(),
        }
    }

    pub fn delete(range: TextRange) -> Edit {
        Edit {
            range,
            replacement: String::new(),
        }
    }
}

#[derive(Debug)]
pub struct RuleMeta {
    /// Stable kebab-case identifier, used in configuration and output.
    pub id: &'static str,
    pub summary: &'static str,
    /// The Checkstyle check this rule mirrors, if any.
    pub checkstyle: Option<&'static str>,
    /// The PMD rule this rule mirrors, if any.
    pub pmd: Option<&'static str>,
    /// Whether the rule can offer a fix for at least some violations.
    pub fixable: bool,
}

/// A lint rule. The linter walks the tree once and offers every node and
/// token to every enabled rule.
pub trait Rule: Sync {
    fn meta(&self) -> &'static RuleMeta;

    fn check_node(&self, _node: &SyntaxNode, _out: &mut Vec<Diagnostic>) {}

    fn check_token(&self, _token: &SyntaxToken, _out: &mut Vec<Diagnostic>) {}
}

/// Every rule WeaveJig implements, in id order.
pub fn all_rules() -> &'static [&'static dyn Rule] {
    rules::ALL
}

/// The rules enabled for a run.
#[derive(Clone)]
pub struct RuleSet {
    rules: Vec<&'static dyn Rule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownRule(pub String);

impl std::fmt::Display for UnknownRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown lint rule `{}`", self.0)
    }
}

impl std::error::Error for UnknownRule {}

impl RuleSet {
    pub fn all() -> RuleSet {
        RuleSet {
            rules: all_rules().to_vec(),
        }
    }

    /// Builds a rule set from configuration. `select` of `None` means every
    /// rule; `ignore` is then removed from the selection.
    pub fn from_selection(
        select: Option<&[String]>,
        ignore: &[String],
    ) -> Result<RuleSet, UnknownRule> {
        let known = |id: &String| all_rules().iter().any(|rule| rule.meta().id == id);
        if let Some(unknown) = select
            .unwrap_or_default()
            .iter()
            .chain(ignore)
            .find(|id| !known(id))
        {
            return Err(UnknownRule(unknown.clone()));
        }
        let rules = all_rules()
            .iter()
            .copied()
            .filter(|rule| {
                let id = rule.meta().id;
                select.is_none_or(|select| select.iter().any(|s| s == id))
                    && !ignore.iter().any(|i| i == id)
            })
            .collect();
        Ok(RuleSet { rules })
    }

    pub fn iter(&self) -> impl Iterator<Item = &'static dyn Rule> + '_ {
        self.rules.iter().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

/// Lints Java source text.
///
/// If the text does not parse, the result holds only [`SYNTAX_ERROR`]
/// diagnostics: rules are not run on trees that contain errors, since their
/// findings there would be unreliable.
pub fn lint(text: &str, rules: &RuleSet) -> Vec<Diagnostic> {
    let parse = parse(text);
    if !parse.is_ok() {
        return parse
            .errors()
            .iter()
            .map(|error| Diagnostic {
                rule: SYNTAX_ERROR,
                message: error.message.clone(),
                range: error.range,
                fix: None,
            })
            .collect();
    }
    lint_tree(&parse.syntax(), rules)
}

/// Runs `rules` over an error-free syntax tree.
pub fn lint_tree(root: &SyntaxNode, rules: &RuleSet) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if rules.is_empty() {
        return diagnostics;
    }
    for event in root.preorder_with_tokens() {
        let WalkEvent::Enter(element) = event else {
            continue;
        };
        match element {
            NodeOrToken::Node(node) => {
                for rule in rules.iter() {
                    rule.check_node(&node, &mut diagnostics);
                }
            }
            NodeOrToken::Token(token) if !token.kind().is_trivia() => {
                for rule in rules.iter() {
                    rule.check_token(&token, &mut diagnostics);
                }
            }
            NodeOrToken::Token(_) => {}
        }
    }
    diagnostics.sort_by_key(|d| (d.range.start(), d.range.end(), d.rule));
    diagnostics
}
