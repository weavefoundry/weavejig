//! Syntax-level lint rules for Java, with automatic fixes.
//!
//! Rules are reimplemented from the documented and observed behaviour of
//! Checkstyle and PMD. Each rule records which reference check it mirrors so
//! the scoring harness can compare diagnostics rule by rule.
//!
//! A run is described by a [`RuleSet`]: which rules are on, and the value of
//! each of their options. [`lint`] parses a source file, offers every node
//! and token of its tree to every enabled rule, and drops the diagnostics
//! that the source itself suppresses (see [`suppress`]).

mod ast;
mod context;
mod fix;
mod javadoc;
mod options;
mod rules;
mod ruleset;
mod suppress;

use weavejig_syntax::{LineIndex, NodeOrToken, SyntaxNode, SyntaxToken, TextRange, WalkEvent};

pub use crate::context::{Context, Source};
pub use crate::fix::{FixOutcome, apply_fixes, fix_all};
pub use crate::options::{OptionDefault, OptionError, OptionSpec, Options, RawValue};
pub use crate::ruleset::{ALL_RULES, RuleSet, Selection, SelectionError};
pub use crate::suppress::SuppressionNames;

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

impl Diagnostic {
    pub fn new(
        meta: &'static RuleMeta,
        range: TextRange,
        message: impl Into<String>,
    ) -> Diagnostic {
        Diagnostic {
            rule: meta.id,
            message: message.into(),
            range,
            fix: None,
        }
    }

    pub fn with_fix(mut self, description: impl Into<String>, edits: Vec<Edit>) -> Diagnostic {
        self.fix = Some(Fix {
            description: description.into(),
            edits,
        });
        self
    }
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

/// A PMD rule, by the category it is in and its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PmdRule {
    pub category: &'static str,
    pub name: &'static str,
}

#[derive(Debug)]
pub struct RuleMeta {
    /// Stable kebab-case identifier, used in configuration and output.
    pub id: &'static str,
    pub summary: &'static str,
    /// The Checkstyle check this rule mirrors, if any.
    pub checkstyle: Option<&'static str>,
    /// The PMD rule this rule mirrors, if any.
    pub pmd: Option<PmdRule>,
    /// Whether the rule can offer a fix for at least some violations.
    pub fixable: bool,
    /// Whether the rule runs when the configuration selects nothing. Rules
    /// that are a matter of taste, or noisy on ordinary code, are opt-in.
    pub default: bool,
    pub options: &'static [OptionSpec],
}

impl RuleMeta {
    /// A rule that is on by default, mirrors nothing, and has no fix and no
    /// options. The methods below change that.
    pub const fn new(id: &'static str, summary: &'static str) -> RuleMeta {
        RuleMeta {
            id,
            summary,
            checkstyle: None,
            pmd: None,
            fixable: false,
            default: true,
            options: &[],
        }
    }

    pub const fn checkstyle(mut self, check: &'static str) -> RuleMeta {
        self.checkstyle = Some(check);
        self
    }

    /// `category` is the PMD rule category, as in its ruleset file's name:
    /// `codestyle` for `category/java/codestyle.xml`.
    pub const fn pmd(mut self, category: &'static str, name: &'static str) -> RuleMeta {
        self.pmd = Some(PmdRule { category, name });
        self
    }

    pub const fn fixable(mut self) -> RuleMeta {
        self.fixable = true;
        self
    }

    /// Leaves the rule out of the default selection.
    pub const fn opt_in(mut self) -> RuleMeta {
        self.default = false;
        self
    }

    pub const fn options(mut self, options: &'static [OptionSpec]) -> RuleMeta {
        self.options = options;
        self
    }
}

/// A lint rule. The linter walks the tree once and offers every node and
/// token to every enabled rule.
pub trait Rule: Sync {
    fn meta(&self) -> &'static RuleMeta;

    /// Called once per file, before the tree is walked. For rules about the
    /// file as a whole, or that need to look at all of it at once.
    fn check_file(&self, _ctx: &Context<'_>, _out: &mut Vec<Diagnostic>) {}

    fn check_node(&self, _node: &SyntaxNode, _ctx: &Context<'_>, _out: &mut Vec<Diagnostic>) {}

    /// Whether the rule looks at tokens. Most rules are about nodes, and
    /// most of a tree is tokens, so only the rules that ask are offered them.
    fn checks_tokens(&self) -> bool {
        false
    }

    /// Called for every token that is not whitespace or a comment, if
    /// [`checks_tokens`](Rule::checks_tokens) says so.
    fn check_token(&self, _token: &SyntaxToken, _ctx: &Context<'_>, _out: &mut Vec<Diagnostic>) {}
}

/// Every rule WeaveJig implements, in id order.
pub fn all_rules() -> &'static [&'static dyn Rule] {
    rules::ALL
}

/// Lints Java source text.
///
/// If the text does not parse, the result holds only [`SYNTAX_ERROR`]
/// diagnostics: rules are not run on trees that contain errors, since their
/// findings there would be unreliable.
pub fn lint<'a>(source: impl Into<Source<'a>>, rules: &RuleSet) -> Vec<Diagnostic> {
    let source = source.into();
    let parse = weavejig_syntax::parse(source.text);
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
    lint_tree(&parse.syntax(), source, rules)
}

/// Runs `rules` over the error-free syntax tree of `source`.
pub fn lint_tree(root: &SyntaxNode, source: Source<'_>, rules: &RuleSet) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if rules.is_empty() {
        return diagnostics;
    }
    let lines = LineIndex::new(source.text);
    let contexts: Vec<(&'static dyn Rule, Context<'_>)> = rules
        .entries()
        .map(|(rule, options)| (rule, Context::new(source, root, &lines, options)))
        .collect();
    for (rule, ctx) in &contexts {
        rule.check_file(ctx, &mut diagnostics);
    }
    let token_rules: Vec<&(&'static dyn Rule, Context<'_>)> = contexts
        .iter()
        .filter(|(rule, _)| rule.checks_tokens())
        .collect();
    for event in root.preorder_with_tokens() {
        let WalkEvent::Enter(element) = event else {
            continue;
        };
        match element {
            NodeOrToken::Node(node) => {
                for (rule, ctx) in &contexts {
                    rule.check_node(&node, ctx, &mut diagnostics);
                }
            }
            NodeOrToken::Token(token) if !token.kind().is_trivia() => {
                for (rule, ctx) in &token_rules {
                    rule.check_token(&token, ctx, &mut diagnostics);
                }
            }
            NodeOrToken::Token(_) => {}
        }
    }
    let suppressions =
        suppress::Suppressions::collect(root, &lines, source.text, rules.suppression_names());
    if !suppressions.is_empty() {
        diagnostics.retain(|diagnostic| {
            let Some(meta) = rules.meta(diagnostic.rule) else {
                return true;
            };
            let line = lines.line_col(source.text, diagnostic.range.start()).line;
            !suppressions.covers(meta, diagnostic.range.start(), line)
        });
    }
    diagnostics.sort_by_key(|d| (d.range.start(), d.range.end(), d.rule));
    diagnostics
}
