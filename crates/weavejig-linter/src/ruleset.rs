//! Which rules a run uses, and with what options.

use std::sync::Arc;

use crate::suppress::SuppressionNames;
use crate::{OptionError, Options, RawValue, Rule, RuleMeta, all_rules};

/// In a selection, stands for every rule.
pub const ALL_RULES: &str = "all";

/// How configuration chooses rules.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    /// The rules to run. `None` is the default set: every rule that is not
    /// opt-in. [`ALL_RULES`] stands for every rule.
    pub select: Option<Vec<String>>,
    /// Rules to run as well as those of `select`.
    pub extend_select: Vec<String>,
    /// Rules to skip, applied last.
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionError {
    UnknownRule(String),
    Option(OptionError),
}

impl std::fmt::Display for SelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SelectionError::UnknownRule(id) => write!(f, "unknown lint rule `{id}`"),
            SelectionError::Option(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for SelectionError {}

impl From<OptionError> for SelectionError {
    fn from(error: OptionError) -> SelectionError {
        SelectionError::Option(error)
    }
}

/// The rules enabled for a run, each with its options.
#[derive(Clone)]
pub struct RuleSet {
    rules: Vec<(&'static dyn Rule, Arc<Options>)>,
    names: SuppressionNames,
}

fn find(id: &str) -> Option<&'static dyn Rule> {
    all_rules()
        .iter()
        .copied()
        .find(|rule| rule.meta().id == id)
}

fn with_defaults(rule: &'static dyn Rule) -> (&'static dyn Rule, Arc<Options>) {
    let meta = rule.meta();
    (rule, Arc::new(Options::defaults(meta.id, meta.options)))
}

impl std::fmt::Debug for RuleSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.iter().map(|rule| rule.meta().id))
            .finish()
    }
}

impl RuleSet {
    /// Every rule, with default options.
    pub fn all() -> RuleSet {
        RuleSet {
            rules: all_rules().iter().copied().map(with_defaults).collect(),
            names: SuppressionNames::default(),
        }
    }

    /// The rules that run when nothing is configured.
    pub fn defaults() -> RuleSet {
        RuleSet {
            rules: all_rules()
                .iter()
                .copied()
                .filter(|rule| rule.meta().default)
                .map(with_defaults)
                .collect(),
            names: SuppressionNames::default(),
        }
    }

    /// The rules a [`Selection`] chooses, with default options.
    pub fn new(selection: &Selection) -> Result<RuleSet, SelectionError> {
        let named = selection
            .select
            .iter()
            .flatten()
            .chain(&selection.extend_select)
            .chain(&selection.ignore);
        if let Some(unknown) = named
            .filter(|id| id.as_str() != ALL_RULES)
            .find(|id| find(id).is_none())
        {
            return Err(SelectionError::UnknownRule(unknown.clone()));
        }
        let listed =
            |ids: &[String], meta: &RuleMeta| ids.iter().any(|id| id == meta.id || id == ALL_RULES);
        let rules = all_rules()
            .iter()
            .copied()
            .filter(|rule| {
                let meta = rule.meta();
                let selected = match &selection.select {
                    Some(select) => listed(select, meta),
                    None => meta.default,
                };
                (selected || listed(&selection.extend_select, meta))
                    && !listed(&selection.ignore, meta)
            })
            .map(with_defaults)
            .collect();
        Ok(RuleSet {
            rules,
            names: SuppressionNames::default(),
        })
    }

    /// The rules named in `select` (every default rule if `None`), without
    /// those in `ignore`.
    pub fn from_selection(
        select: Option<&[String]>,
        ignore: &[String],
    ) -> Result<RuleSet, SelectionError> {
        RuleSet::new(&Selection {
            select: select.map(<[String]>::to_vec),
            extend_select: Vec::new(),
            ignore: ignore.to_vec(),
        })
    }

    /// Sets an option of a rule. Options of rules that exist but are not
    /// enabled are checked and then ignored, so that a configuration file can
    /// keep the options of a rule it has switched off.
    pub fn set_option(
        &mut self,
        rule: &str,
        option: &str,
        value: RawValue,
    ) -> Result<(), SelectionError> {
        let Some(known) = find(rule) else {
            return Err(SelectionError::UnknownRule(rule.to_string()));
        };
        let meta = known.meta();
        match self
            .rules
            .iter_mut()
            .find(|(rule, _)| rule.meta().id == meta.id)
        {
            Some((_, options)) => {
                Arc::make_mut(options).set(meta.id, meta.options, option, value)?;
            }
            None => Options::defaults(meta.id, meta.options).set(
                meta.id,
                meta.options,
                option,
                value,
            )?,
        }
        Ok(())
    }

    /// This set without the rules in `ids`.
    pub fn without(&self, ids: &[String]) -> RuleSet {
        RuleSet {
            rules: self
                .rules
                .iter()
                .filter(|(rule, _)| !ids.iter().any(|id| id == rule.meta().id || id == ALL_RULES))
                .cloned()
                .collect(),
            names: self.names,
        }
    }

    /// Chooses which names suppress diagnostics; see [`SuppressionNames`].
    pub fn with_suppression_names(mut self, names: SuppressionNames) -> RuleSet {
        self.names = names;
        self
    }

    pub fn suppression_names(&self) -> SuppressionNames {
        self.names
    }

    pub fn iter(&self) -> impl Iterator<Item = &'static dyn Rule> + '_ {
        self.rules.iter().map(|(rule, _)| *rule)
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = (&'static dyn Rule, &Options)> + '_ {
        self.rules.iter().map(|(rule, options)| (*rule, &**options))
    }

    /// The metadata of an enabled rule.
    pub fn meta(&self, id: &str) -> Option<&'static RuleMeta> {
        self.iter()
            .map(|rule| rule.meta())
            .find(|meta| meta.id == id)
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }
}
