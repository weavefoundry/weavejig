//! Rule options: what a rule can be configured with, and the values a run
//! uses.
//!
//! A rule declares its options in its [`RuleMeta`](crate::RuleMeta), each
//! with a default. Configuration may override them; the result is an
//! [`Options`], which the rule reads through its [`Context`](crate::Context).

use regex::Regex;

/// One option of a rule.
#[derive(Debug)]
pub struct OptionSpec {
    /// Kebab-case name, as written in `weavejig.toml`.
    pub name: &'static str,
    pub default: OptionDefault,
    pub help: &'static str,
}

/// The default value of an option, which also fixes its type.
#[derive(Debug, Clone, Copy)]
pub enum OptionDefault {
    Bool(bool),
    Int(i64),
    Str(&'static str),
    /// A regular expression, compiled once per run.
    Regex(&'static str),
    List(&'static [&'static str]),
}

impl OptionDefault {
    pub fn type_name(&self) -> &'static str {
        match self {
            OptionDefault::Bool(_) => "boolean",
            OptionDefault::Int(_) => "integer",
            OptionDefault::Str(_) => "string",
            OptionDefault::Regex(_) => "regular expression",
            OptionDefault::List(_) => "list of strings",
        }
    }

    /// The type with its article, for messages.
    fn described(&self) -> &'static str {
        match self {
            OptionDefault::Bool(_) => "a boolean",
            OptionDefault::Int(_) => "an integer",
            OptionDefault::Str(_) => "a string",
            OptionDefault::Regex(_) => "a regular expression",
            OptionDefault::List(_) => "a list of strings",
        }
    }

    /// The default as configuration would write it.
    pub fn to_raw(&self) -> RawValue {
        match self {
            OptionDefault::Bool(value) => RawValue::Bool(*value),
            OptionDefault::Int(value) => RawValue::Int(*value),
            OptionDefault::Str(value) | OptionDefault::Regex(value) => {
                RawValue::Str((*value).to_string())
            }
            OptionDefault::List(values) => {
                RawValue::List(values.iter().map(|value| (*value).to_string()).collect())
            }
        }
    }
}

/// A value as configuration gives it, before it is checked against the
/// option's type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawValue {
    Bool(bool),
    Int(i64),
    Str(String),
    List(Vec<String>),
}

#[derive(Debug, Clone)]
enum Value {
    Bool(bool),
    Int(i64),
    Str(String),
    Regex(Regex),
    List(Vec<String>),
}

/// The option values of one rule for one run.
#[derive(Debug, Clone, Default)]
pub struct Options {
    values: Vec<(&'static str, Value)>,
}

/// Why a configured value was not accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionError {
    Unknown {
        rule: &'static str,
        option: String,
        known: Vec<&'static str>,
    },
    WrongType {
        rule: &'static str,
        option: &'static str,
        expected: &'static str,
    },
    BadRegex {
        rule: &'static str,
        option: &'static str,
        error: String,
    },
}

impl std::fmt::Display for OptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionError::Unknown {
                rule,
                option,
                known,
            } if known.is_empty() => {
                write!(
                    f,
                    "rule `{rule}` has no option `{option}`: it has no options"
                )
            }
            OptionError::Unknown {
                rule,
                option,
                known,
            } => write!(
                f,
                "rule `{rule}` has no option `{option}`; its options are {}",
                known
                    .iter()
                    .map(|name| format!("`{name}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            OptionError::WrongType {
                rule,
                option,
                expected,
            } => write!(f, "option `{option}` of rule `{rule}` must be {expected}"),
            OptionError::BadRegex {
                rule,
                option,
                error,
            } => write!(
                f,
                "option `{option}` of rule `{rule}` is not a valid regular expression: {error}"
            ),
        }
    }
}

impl std::error::Error for OptionError {}

fn compile(rule: &'static str, option: &'static str, pattern: &str) -> Result<Regex, OptionError> {
    Regex::new(pattern).map_err(|error| OptionError::BadRegex {
        rule,
        option,
        error: error.to_string(),
    })
}

impl Options {
    /// The defaults of `specs`.
    pub(crate) fn defaults(rule: &'static str, specs: &'static [OptionSpec]) -> Options {
        let values = specs
            .iter()
            .map(|spec| {
                let value = match spec.default {
                    OptionDefault::Bool(value) => Value::Bool(value),
                    OptionDefault::Int(value) => Value::Int(value),
                    OptionDefault::Str(value) => Value::Str(value.to_string()),
                    OptionDefault::Regex(pattern) => Value::Regex(
                        compile(rule, spec.name, pattern)
                            .unwrap_or_else(|error| panic!("default of a rule option: {error}")),
                    ),
                    OptionDefault::List(values) => {
                        Value::List(values.iter().map(|value| (*value).to_string()).collect())
                    }
                };
                (spec.name, value)
            })
            .collect();
        Options { values }
    }

    /// Overrides one option with a configured value.
    pub(crate) fn set(
        &mut self,
        rule: &'static str,
        specs: &'static [OptionSpec],
        option: &str,
        raw: RawValue,
    ) -> Result<(), OptionError> {
        let Some(spec) = specs.iter().find(|spec| spec.name == option) else {
            return Err(OptionError::Unknown {
                rule,
                option: option.to_string(),
                known: specs.iter().map(|spec| spec.name).collect(),
            });
        };
        let wrong_type = || OptionError::WrongType {
            rule,
            option: spec.name,
            expected: spec.default.described(),
        };
        let value = match (spec.default, raw) {
            (OptionDefault::Bool(_), RawValue::Bool(value)) => Value::Bool(value),
            (OptionDefault::Int(_), RawValue::Int(value)) => Value::Int(value),
            (OptionDefault::Str(_), RawValue::Str(value)) => Value::Str(value),
            (OptionDefault::Regex(_), RawValue::Str(pattern)) => {
                Value::Regex(compile(rule, spec.name, &pattern)?)
            }
            (OptionDefault::List(_), RawValue::List(values)) => Value::List(values),
            // One string is a list of one.
            (OptionDefault::List(_), RawValue::Str(value)) => Value::List(vec![value]),
            _ => return Err(wrong_type()),
        };
        match self.values.iter_mut().find(|(name, _)| *name == spec.name) {
            Some(slot) => slot.1 = value,
            None => self.values.push((spec.name, value)),
        }
        Ok(())
    }

    fn get(&self, name: &str) -> Option<&Value> {
        let value = self
            .values
            .iter()
            .find(|(option, _)| *option == name)
            .map(|(_, value)| value);
        debug_assert!(value.is_some(), "rule reads undeclared option `{name}`");
        value
    }

    pub fn bool(&self, name: &str) -> bool {
        matches!(self.get(name), Some(Value::Bool(true)))
    }

    pub fn int(&self, name: &str) -> i64 {
        match self.get(name) {
            Some(Value::Int(value)) => *value,
            _ => 0,
        }
    }

    pub fn str(&self, name: &str) -> &str {
        match self.get(name) {
            Some(Value::Str(value)) => value,
            _ => "",
        }
    }

    pub fn list(&self, name: &str) -> &[String] {
        match self.get(name) {
            Some(Value::List(values)) => values,
            _ => &[],
        }
    }

    /// Whether the regular-expression option `name` matches somewhere in
    /// `text`. Anchor the pattern to match all of it.
    pub fn matches(&self, name: &str, text: &str) -> bool {
        match self.get(name) {
            Some(Value::Regex(regex)) => regex.is_match(text),
            _ => false,
        }
    }

    pub fn regex(&self, name: &str) -> Option<&Regex> {
        match self.get(name) {
            Some(Value::Regex(regex)) => Some(regex),
            _ => None,
        }
    }
}
