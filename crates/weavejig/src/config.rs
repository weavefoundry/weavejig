//! `weavejig.toml`: the one configuration file.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use globset::{GlobBuilder, GlobMatcher};
use serde::Deserialize;
use weavejig_formatter::{FormatOptions, Style};
use weavejig_linter::{ALL_RULES, RawValue, RuleSet, Selection, all_rules};

pub const FILE_NAME: &str = "weavejig.toml";

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
    /// Glob patterns, relative to the configuration file, for paths that are
    /// never checked or formatted.
    pub exclude: Vec<String>,
    pub format: FormatConfig,
    pub lint: LintConfig,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct FormatConfig {
    pub style: StyleName,
}

#[derive(Debug, Default, Clone, Copy, Deserialize, PartialEq, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum StyleName {
    #[default]
    Google,
    Aosp,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct LintConfig {
    /// Rules to run; `"all"` for every rule. The default rules when absent.
    pub select: Option<Vec<String>>,
    /// Rules to run as well as those of `select`.
    pub extend_select: Vec<String>,
    /// Rules to skip, applied last.
    pub ignore: Vec<String>,
    /// Rules to skip in the files a glob matches, relative to this file.
    pub per_file_ignores: BTreeMap<String, Vec<String>>,
    /// Options of rules, by rule id and option name.
    pub rules: BTreeMap<String, BTreeMap<String, toml::Value>>,
}

/// What the command line says about rule selection. Each part replaces the
/// configuration file's when given, and `select` replaces the file's
/// `extend-select` as well.
#[derive(Debug, Default)]
pub struct SelectionOverrides<'a> {
    pub select: &'a [String],
    pub extend_select: &'a [String],
    pub ignore: &'a [String],
}

/// The rules to skip in files matching a glob.
pub struct PerFileIgnores {
    entries: Vec<(GlobMatcher, Vec<String>)>,
    root: PathBuf,
}

impl PerFileIgnores {
    /// The rules to skip for `path`, which is relative to the current
    /// directory or absolute.
    pub fn for_path(&self, path: &Path) -> Vec<String> {
        if self.entries.is_empty() {
            return Vec::new();
        }
        let absolute = crate::files::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        let relative = absolute.strip_prefix(&self.root).unwrap_or(&absolute);
        self.entries
            .iter()
            .filter(|(glob, _)| glob.is_match(relative))
            .flat_map(|(_, rules)| rules.iter().cloned())
            .collect()
    }
}

/// A TOML value as a rule option.
fn raw_value(value: &toml::Value) -> Option<RawValue> {
    let scalar = |value: &toml::Value| match value {
        toml::Value::String(text) => Some(text.clone()),
        // Numbers in a list of strings, as in `ignore-numbers = [0, 1]`.
        toml::Value::Integer(number) => Some(number.to_string()),
        toml::Value::Float(number) => Some(number.to_string()),
        _ => None,
    };
    match value {
        toml::Value::Boolean(value) => Some(RawValue::Bool(*value)),
        toml::Value::Integer(value) => Some(RawValue::Int(*value)),
        toml::Value::String(value) => Some(RawValue::Str(value.clone())),
        toml::Value::Array(values) => values
            .iter()
            .map(scalar)
            .collect::<Option<_>>()
            .map(RawValue::List),
        _ => None,
    }
}

/// A loaded configuration and the directory its relative paths resolve from.
#[derive(Debug)]
pub struct Loaded {
    pub config: Config,
    pub root: PathBuf,
}

impl Config {
    pub fn parse(text: &str) -> Result<Config> {
        Ok(toml::from_str(text)?)
    }

    pub fn format_options(&self, style: Option<StyleName>) -> FormatOptions {
        let style = match style.unwrap_or(self.format.style) {
            StyleName::Google => Style::Google,
            StyleName::Aosp => Style::Aosp,
        };
        FormatOptions { style }
    }

    /// The enabled rules with their options. Command-line selections
    /// replace the file's.
    pub fn rules(&self, overrides: &SelectionOverrides<'_>) -> Result<RuleSet> {
        let or_file = |given: &[String], file: &[String]| {
            if given.is_empty() {
                file.to_vec()
            } else {
                given.to_vec()
            }
        };
        let selection = Selection {
            select: if overrides.select.is_empty() {
                self.lint.select.clone()
            } else {
                Some(overrides.select.to_vec())
            },
            // `--select` means these rules and no others, so it replaces
            // the file's `extend-select` too.
            extend_select: if overrides.select.is_empty() {
                or_file(overrides.extend_select, &self.lint.extend_select)
            } else {
                overrides.extend_select.to_vec()
            },
            ignore: or_file(overrides.ignore, &self.lint.ignore),
        };
        let mut rules = RuleSet::new(&selection)?;
        for (rule, options) in &self.lint.rules {
            for (option, value) in options {
                let Some(value) = raw_value(value) else {
                    bail!(
                        "option `{option}` of rule `{rule}` must be a boolean, an integer, a string or a list of strings"
                    );
                };
                rules.set_option(rule, option, value)?;
            }
        }
        Ok(rules)
    }

    /// The per-file ignores, with globs relative to `root`.
    pub fn per_file_ignores(&self, root: &Path) -> Result<PerFileIgnores> {
        let mut entries = Vec::new();
        for (pattern, rules) in &self.lint.per_file_ignores {
            if let Some(unknown) = rules.iter().find(|rule| {
                rule.as_str() != ALL_RULES
                    && !all_rules().iter().any(|known| known.meta().id == *rule)
            }) {
                bail!("unknown lint rule `{unknown}` in `per-file-ignores`");
            }
            // As in `.gitignore`: `*` stays within a directory, `**` does not.
            let glob = GlobBuilder::new(pattern)
                .literal_separator(true)
                .build()
                .with_context(|| format!("invalid `per-file-ignores` pattern `{pattern}`"))?;
            entries.push((glob.compile_matcher(), rules.clone()));
        }
        // With `..` resolved, as the files' paths are, so that the two can
        // be compared.
        let root = crate::files::absolute(root).unwrap_or_else(|_| root.to_path_buf());
        Ok(PerFileIgnores { entries, root })
    }
}

/// Loads `explicit` if given; otherwise the nearest `weavejig.toml` in
/// `start` or a directory above it; otherwise the defaults.
pub fn load(explicit: Option<&Path>, start: &Path) -> Result<Loaded> {
    let path = match explicit {
        Some(path) => Some(path.to_path_buf()),
        None => start
            .ancestors()
            .map(|dir| dir.join(FILE_NAME))
            .find(|path| path.is_file()),
    };
    let Some(path) = path else {
        return Ok(Loaded {
            config: Config::default(),
            root: start.to_path_buf(),
        });
    };
    let text = fs::read_to_string(&path)
        .with_context(|| format!("cannot read configuration file {}", path.display()))?;
    let config = Config::parse(&text)
        .with_context(|| format!("invalid configuration file {}", path.display()))?;
    let root = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    Ok(Loaded { config, root })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_is_the_default() {
        assert_eq!(Config::parse("").unwrap(), Config::default());
    }

    #[test]
    fn full_file() {
        let config = Config::parse(
            r#"
            exclude = ["**/generated/**"]

            [format]
            style = "aosp"

            [lint]
            select = ["upper-ell", "need-braces"]
            ignore = ["need-braces"]
            "#,
        )
        .unwrap();
        assert_eq!(config.exclude, ["**/generated/**"]);
        assert_eq!(config.format_options(None).style, Style::Aosp);
        assert_eq!(
            config.format_options(Some(StyleName::Google)).style,
            Style::Google
        );
        let ids = |overrides: &SelectionOverrides<'_>| -> Vec<&'static str> {
            let rules = config.rules(overrides).unwrap();
            rules.iter().map(|r| r.meta().id).collect()
        };
        assert_eq!(ids(&SelectionOverrides::default()), ["upper-ell"]);
        // The command line replaces the file's selection.
        let overrides = SelectionOverrides {
            select: &["need-braces".to_string()],
            ignore: &["upper-ell".to_string()],
            ..SelectionOverrides::default()
        };
        assert_eq!(ids(&overrides), ["need-braces"]);
    }

    #[test]
    fn selection_extends_the_defaults() {
        let defaults = Config::default()
            .rules(&SelectionOverrides::default())
            .unwrap();
        assert!(defaults.meta("need-braces").is_some());
        // Opt-in rules are not in the default set.
        assert!(defaults.meta("magic-number").is_none());

        let config = Config::parse("[lint]\nextend-select = [\"magic-number\"]").unwrap();
        let extended = config.rules(&SelectionOverrides::default()).unwrap();
        assert_eq!(extended.len(), defaults.len() + 1);
        assert!(extended.meta("magic-number").is_some());

        // `--select` runs only what it names: the file's `extend-select`
        // goes with the file's `select`.
        let only = |overrides: &SelectionOverrides<'_>| -> Vec<&'static str> {
            let rules = config.rules(overrides).unwrap();
            rules.iter().map(|rule| rule.meta().id).collect()
        };
        let select = ["upper-ell".to_string()];
        let extend = ["need-braces".to_string()];
        assert_eq!(
            only(&SelectionOverrides {
                select: &select,
                ..SelectionOverrides::default()
            }),
            ["upper-ell"]
        );
        assert_eq!(
            only(&SelectionOverrides {
                select: &select,
                extend_select: &extend,
                ..SelectionOverrides::default()
            }),
            ["need-braces", "upper-ell"]
        );

        let config =
            Config::parse("[lint]\nselect = [\"all\"]\nignore = [\"magic-number\"]").unwrap();
        let all = config.rules(&SelectionOverrides::default()).unwrap();
        assert_eq!(all.len(), all_rules().len() - 1);
    }

    #[test]
    fn rule_options() {
        let config = Config::parse(
            r#"
            [lint]
            select = ["line-length", "magic-number"]

            [lint.rules.line-length]
            max = 120

            [lint.rules.magic-number]
            ignore-numbers = [0, 1, "0x10"]
            ignore-hash-code-method = true
            "#,
        )
        .unwrap();
        let rules = config.rules(&SelectionOverrides::default()).unwrap();
        let long = format!(
            "class A {{\n  // {}\n  int n = 16;\n  int m = 7;\n}}\n",
            "x".repeat(110)
        );
        let lines: Vec<_> = weavejig_linter::lint(&long, &rules)
            .iter()
            .map(|d| d.rule)
            .collect();
        // 115 columns fit in 120, and 16 is ignored as `0x10`.
        assert_eq!(lines, ["magic-number"]);

        let bad = |text: &str| {
            let config = Config::parse(text).unwrap();
            format!(
                "{:#}",
                config.rules(&SelectionOverrides::default()).unwrap_err()
            )
        };
        assert!(bad("[lint.rules.line-length]\nmaxx = 1").contains("has no option `maxx`"));
        assert!(bad("[lint.rules.line-length]\nmax = \"wide\"").contains("must be an integer"));
        assert!(bad("[lint.rules.no-such-rule]\nmax = 1").contains("unknown lint rule"));
        assert!(bad("[lint.rules.todo-comment]\nformat = \"(\"").contains("regular expression"));
    }

    #[test]
    fn per_file_ignores() {
        let config = Config::parse(
            "[lint.per-file-ignores]\n\"**/test/**\" = [\"magic-number\"]\n\"*.java\" = [\"all\"]",
        )
        .unwrap();
        let ignores = config.per_file_ignores(Path::new("/repo")).unwrap();
        assert_eq!(
            ignores.for_path(Path::new("/repo/src/test/java/FooTest.java")),
            ["magic-number"]
        );
        assert_eq!(ignores.for_path(Path::new("/repo/Top.java")), ["all"]);
        assert!(
            ignores
                .for_path(Path::new("/repo/src/main/Foo.java"))
                .is_empty()
        );

        // `..` in the root or in a path does not stop a glob from matching.
        let ignores = config.per_file_ignores(Path::new("/repo/sub/..")).unwrap();
        assert_eq!(
            ignores.for_path(Path::new("/repo/src/test/java/FooTest.java")),
            ["magic-number"]
        );
        assert_eq!(
            ignores.for_path(Path::new("/repo/sub/../Top.java")),
            ["all"]
        );

        let config = Config::parse("[lint.per-file-ignores]\n\"**\" = [\"nope\"]").unwrap();
        assert!(config.per_file_ignores(Path::new(".")).is_err());
    }

    #[test]
    fn mistakes_are_reported() {
        assert!(Config::parse("[lint]\nselcet = []").is_err());
        assert!(Config::parse("[format]\nstyle = \"sun\"").is_err());
        let config = Config::parse("[lint]\nselect = [\"no-such-rule\"]").unwrap();
        assert!(config.rules(&SelectionOverrides::default()).is_err());
    }
}
