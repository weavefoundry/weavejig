//! `weavejig.toml`: the one configuration file.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;
use weavejig_formatter::{FormatOptions, Style};
use weavejig_linter::RuleSet;

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
    /// Rules to run. All rules when absent.
    pub select: Option<Vec<String>>,
    /// Rules to skip, applied after `select`.
    pub ignore: Vec<String>,
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

    /// The enabled rules. Command-line selections replace the file's.
    pub fn rules(&self, select: &[String], ignore: &[String]) -> Result<RuleSet> {
        let select = if select.is_empty() {
            self.lint.select.as_deref()
        } else {
            Some(select)
        };
        let ignore = if ignore.is_empty() {
            self.lint.ignore.as_slice()
        } else {
            ignore
        };
        Ok(RuleSet::from_selection(select, ignore)?)
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
        let ids: Vec<_> = config
            .rules(&[], &[])
            .unwrap()
            .iter()
            .map(|r| r.meta().id)
            .collect();
        assert_eq!(ids, ["upper-ell"]);
        // The command line replaces the file's selection.
        let ids: Vec<_> = config
            .rules(&["need-braces".to_string()], &["upper-ell".to_string()])
            .unwrap()
            .iter()
            .map(|r| r.meta().id)
            .collect();
        assert_eq!(ids, ["need-braces"]);
    }

    #[test]
    fn mistakes_are_reported() {
        assert!(Config::parse("[lint]\nselcet = []").is_err());
        assert!(Config::parse("[format]\nstyle = \"sun\"").is_err());
        let config = Config::parse("[lint]\nselect = [\"no-such-rule\"]").unwrap();
        assert!(config.rules(&[], &[]).is_err());
    }
}
