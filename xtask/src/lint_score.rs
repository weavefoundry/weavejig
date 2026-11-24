//! The linter scores: WeaveJig's diagnostics against Checkstyle's and PMD's.
//!
//! Each reference tool is run with exactly the checks WeaveJig mirrors, set
//! up to behave as WeaveJig's rules do by default. A diagnostic matches when
//! both tools report the same rule on the same line of the same file.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::Serialize;
use sha2::{Digest, Sha256};
use weavejig_linter::{PmdRule, RuleMeta, RuleSet, SuppressionNames, all_rules, lint_tree};
use weavejig_syntax::{LineIndex, parse};

use crate::java::{self, Jdk};
use crate::manifest::{Manifest, cache_dir};
use crate::score::{CHUNK, Score, Source, display, scratch_dir};

/// How one rule compares with the check it mirrors.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RuleScore {
    pub rule: &'static str,
    /// The reference tool's name for it.
    pub mirrors: &'static str,
    pub matched: usize,
    /// Reported by the reference only.
    pub missed: usize,
    /// Reported by WeaveJig only.
    pub extra: usize,
}

impl RuleScore {
    pub fn percent(&self) -> Option<f64> {
        let total = self.matched + self.missed + self.extra;
        (total > 0).then(|| 100.0 * self.matched as f64 / total as f64)
    }
}

/// What a reference tool reported.
#[derive(Debug, Default, PartialEq)]
pub struct Report {
    /// (index of the source file, the tool's name for the check, line).
    pub violations: Vec<(usize, String, u32)>,
    /// Files the tool could not process.
    pub failed: BTreeSet<usize>,
}

pub struct Settings<'a> {
    /// Reuse a reference tool's earlier output for the same checks and files.
    pub use_cache: bool,
    /// List disagreements of this rule only.
    pub only_rule: Option<&'a str>,
}

// --- Caching ----------------------------------------------------------------
//
// A reference tool's output depends only on its version, its configuration
// and the files, so it is kept between runs: starting a JVM on a whole
// corpus is most of what scoring the linter costs.

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Part of every cache key. Change it when a change to this file makes
/// reports cached earlier wrong, such as reading a tool's output differently.
const CACHE_VERSION: &str = "4";

/// Identifies a run of `tool` with `config` over `sources`.
fn cache_key(tool: &str, config: &str, sources: &[Source]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CACHE_VERSION.as_bytes());
    hasher.update([0]);
    hasher.update(tool.as_bytes());
    hasher.update([0]);
    hasher.update(config.as_bytes());
    for source in sources {
        hasher.update([0]);
        hasher.update(source.path.as_os_str().as_encoded_bytes());
        hasher.update([0]);
        hasher.update(Sha256::digest(source.text.as_bytes()));
    }
    hex(&hasher.finalize()[..16])
}

fn cache_path(key: &str) -> PathBuf {
    cache_dir().join("cache").join(format!("{key}.tsv"))
}

fn read_cache(path: &Path) -> Option<Report> {
    let text = fs::read_to_string(path).ok()?;
    let mut report = Report::default();
    for line in text.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        match fields.as_slice() {
            ["failed", file] => {
                report.failed.insert(file.parse().ok()?);
            }
            ["violation", file, check, line] => {
                report.violations.push((
                    file.parse().ok()?,
                    (*check).to_string(),
                    line.parse().ok()?,
                ));
            }
            _ => return None,
        }
    }
    Some(report)
}

fn write_cache(path: &Path, report: &Report) -> Result<()> {
    let mut text = String::new();
    for file in &report.failed {
        text.push_str(&format!("failed\t{file}\n"));
    }
    for (file, check, line) in &report.violations {
        text.push_str(&format!("violation\t{file}\t{check}\t{line}\n"));
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, text).with_context(|| format!("cannot write {}", path.display()))
}

/// Runs `run`, or returns what it reported last time for the same `key`.
fn cached(
    key: &str,
    settings: &Settings<'_>,
    run: impl FnOnce() -> Result<Result<Report, String>>,
) -> Result<Result<Report, String>> {
    let path = cache_path(key);
    if settings.use_cache
        && let Some(report) = read_cache(&path)
    {
        return Ok(Ok(report));
    }
    let outcome = run()?;
    if let Ok(report) = &outcome {
        write_cache(&path, report)?;
    }
    Ok(outcome)
}

// --- Comparing --------------------------------------------------------------

/// Which line of a WeaveJig diagnostic a reference diagnostic must be on.
///
/// Checkstyle and PMD often report the same finding on different lines:
/// for an `if` without braces, Checkstyle names the `if` and PMD the
/// statement under it. WeaveJig's diagnostics start where Checkstyle's are
/// and cover where PMD's are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineMatch {
    /// The line the diagnostic starts on.
    Start,
    /// Any line the diagnostic covers.
    Within,
}

/// A WeaveJig diagnostic: the lines it covers.
type Ours = (u32, u32);

/// Scores WeaveJig's `rules` against what a reference tool reported.
/// `rule_for` maps the tool's check names to WeaveJig's rules.
fn compare(
    sources: &[Source],
    reference: String,
    report: &Report,
    rule_for: &HashMap<&'static str, &'static RuleMeta>,
    rules: &RuleSet,
    line_match: LineMatch,
    settings: &Settings<'_>,
) -> Score {
    // Only files both tools could process are compared.
    let ours_per_file: Vec<Option<Vec<(&'static str, Ours)>>> = sources
        .par_iter()
        .enumerate()
        .map(|(i, source)| {
            let parse = parse(&source.text);
            if !parse.is_ok() || report.failed.contains(&i) {
                return None;
            }
            let lines = LineIndex::new(&source.text);
            let line = |offset| lines.line_col(&source.text, offset).line;
            let source_file = weavejig_linter::Source::new(&source.text, Some(&source.path));
            let diagnostics = lint_tree(&parse.syntax(), source_file, rules);
            Some(
                diagnostics
                    .iter()
                    .map(|d| (d.rule, (line(d.range.start()), line(d.range.end()))))
                    .collect(),
            )
        })
        .collect();

    // Per file and rule: our diagnostics by starting line, and the lines
    // the reference reports. Several diagnostics on one line count once.
    let mut ours: BTreeMap<(usize, &'static str), BTreeMap<u32, u32>> = BTreeMap::new();
    for (i, diagnostics) in ours_per_file.iter().enumerate() {
        for (rule, (start, end)) in diagnostics.iter().flatten() {
            let last = ours
                .entry((i, *rule))
                .or_default()
                .entry(*start)
                .or_insert(*end);
            *last = (*last).max(*end);
        }
    }
    let mut theirs: BTreeMap<(usize, &'static str), BTreeSet<u32>> = BTreeMap::new();
    for (file, check, line) in &report.violations {
        let Some(meta) = rule_for.get(check.as_str()) else {
            continue;
        };
        if ours_per_file.get(*file).is_some_and(Option::is_some) {
            theirs.entry((*file, meta.id)).or_default().insert(*line);
        }
    }

    let mut per_rule: BTreeMap<&'static str, RuleScore> = rule_for
        .iter()
        .map(|(check, meta)| {
            let score = RuleScore {
                rule: meta.id,
                mirrors: check,
                matched: 0,
                missed: 0,
                extra: 0,
            };
            (meta.id, score)
        })
        .collect();
    let mut mismatches = Vec::new();
    let listed = |rule: &str| settings.only_rule.is_none_or(|only| only == rule);
    let keys: BTreeSet<(usize, &'static str)> = ours.keys().chain(theirs.keys()).copied().collect();
    for key @ (i, rule) in keys {
        let Some(score) = per_rule.get_mut(rule) else {
            continue;
        };
        let mut unmatched = ours.remove(&key).unwrap_or_default();
        let reference_lines = theirs.remove(&key).unwrap_or_default();
        let mut missed = Vec::new();
        // Exact lines first, so that a diagnostic covering several lines
        // does not take the match of one that starts on the line itself.
        for line in &reference_lines {
            if unmatched.remove(line).is_some() {
                score.matched += 1;
            } else {
                missed.push(*line);
            }
        }
        if line_match == LineMatch::Within {
            missed.retain(|line| {
                let covering = unmatched
                    .iter()
                    .find(|(start, end)| *start <= line && line <= *end)
                    .map(|(start, _)| *start);
                match covering {
                    Some(start) => {
                        unmatched.remove(&start);
                        score.matched += 1;
                        false
                    }
                    None => true,
                }
            });
        }
        score.missed += missed.len();
        score.extra += unmatched.len();
        if listed(rule) {
            let path = display(&sources[i].path);
            for line in missed {
                mismatches.push(format!(
                    "{path}:{line}: {rule}: only the reference reports this"
                ));
            }
            for line in unmatched.keys() {
                mismatches.push(format!("{path}:{line}: {rule}: only WeaveJig reports this"));
            }
        }
    }
    mismatches.sort();

    let rules: Vec<RuleScore> = per_rule.into_values().collect();
    let (matched, missed, extra) = rules.iter().fold((0, 0, 0), |(a, b, c), rule| {
        (a + rule.matched, b + rule.missed, c + rule.extra)
    });
    let total = matched + missed + extra;
    let detail = format!(
        "{matched}/{total} diagnostics match over {} rules ({missed} missed, {extra} extra)",
        rules.len()
    );
    let mut score = Score::measured("Linter", reference, matched, total, detail, mismatches);
    score.rules = rules;
    score
}

/// The rules that mirror a check of one tool, by the tool's name for it.
fn mirrored(
    name_of: impl Fn(&RuleMeta) -> Option<&'static str>,
) -> HashMap<&'static str, &'static RuleMeta> {
    all_rules()
        .iter()
        .filter_map(|rule| Some((name_of(rule.meta())?, rule.meta())))
        .collect()
}

/// Exactly the rules in `rule_for`, with their default options.
fn rule_set(
    rule_for: &HashMap<&'static str, &'static RuleMeta>,
    names: SuppressionNames,
) -> Result<RuleSet> {
    let ids: Vec<String> = rule_for.values().map(|meta| meta.id.to_string()).collect();
    Ok(RuleSet::from_selection(Some(&ids), &[])?.with_suppression_names(names))
}

// --- Checkstyle ---------------------------------------------------------------

fn xml_unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn xml_attribute(tag: &str, name: &str) -> Option<String> {
    let start = tag.find(&format!(" {name}=\""))? + name.len() + 3;
    let end = start + tag[start..].find('"')?;
    Some(xml_unescape(&tag[start..end]))
}

#[derive(Debug, Default, PartialEq)]
struct CheckstyleReport {
    /// (file, check name without the `Check` suffix, line).
    violations: Vec<(String, String, u32)>,
    /// Files Checkstyle could not process.
    failed: HashSet<String>,
    /// Files the report mentions at all.
    seen: HashSet<String>,
}

/// Reads Checkstyle's XML report (`-f xml`).
fn parse_checkstyle_xml(xml: &str, report: &mut CheckstyleReport) {
    let mut file = String::new();
    for tag in xml.split('<').skip(1) {
        if tag.starts_with("file ") {
            file = xml_attribute(tag, "name").unwrap_or_default();
            report.seen.insert(file.clone());
        } else if tag.starts_with("exception") {
            report.failed.insert(file.clone());
        } else if tag.starts_with("error ") {
            let source = xml_attribute(tag, "source").unwrap_or_default();
            let check = source
                .rsplit('.')
                .next()
                .and_then(|name| name.strip_suffix("Check"));
            let line = xml_attribute(tag, "line").and_then(|line| line.parse().ok());
            // Checks that read Javadoc report a comment they cannot parse
            // under their own name. That is not a finding of the check.
            let message = xml_attribute(tag, "message").unwrap_or_default();
            if message.starts_with("Javadoc comment at column") && message.contains("parse error") {
                continue;
            }
            match (check, line) {
                (Some(check), Some(line)) => {
                    report
                        .violations
                        .push((file.clone(), check.to_string(), line));
                }
                // An error that is not from a check means the file failed.
                _ => {
                    report.failed.insert(file.clone());
                }
            }
        }
    }
}

/// Checks that are children of `Checker` rather than of `TreeWalker`: they
/// work on the file's text, not its syntax tree.
const FILE_CHECKS: [&str; 6] = [
    "FileLength",
    "FileTabCharacter",
    "LineLength",
    "NewlineAtEndOfFile",
    "RegexpMultiline",
    "RegexpSingleline",
];

/// Properties that make a Checkstyle check behave as the WeaveJig rule that
/// mirrors it does by default, where WeaveJig's default is deliberately not
/// Checkstyle's.
const CHECK_PROPERTIES: [(&str, &[(&str, &str)]); 1] = [
    // WeaveJig's limit is the formatter's line width.
    ("LineLength", &[("max", "100")]),
];

/// A Checkstyle configuration enabling exactly the checks WeaveJig mirrors.
/// The filters make Checkstyle honour the suppressions WeaveJig honours:
/// `@SuppressWarnings("checkstyle:...")` and `CHECKSTYLE:OFF` comments.
fn checkstyle_config(checks: &[&str]) -> String {
    let module = |check: &str, indent: &str| {
        let properties = CHECK_PROPERTIES
            .iter()
            .find(|(name, _)| *name == check)
            .map_or(&[][..], |(_, properties)| properties);
        if properties.is_empty() {
            return format!("{indent}<module name=\"{check}\"/>\n");
        }
        let mut xml = format!("{indent}<module name=\"{check}\">\n");
        for (name, value) in properties {
            xml.push_str(&format!(
                "{indent}  <property name=\"{name}\" value=\"{}\"/>\n",
                xml_escape(value)
            ));
        }
        xml.push_str(&format!("{indent}</module>\n"));
        xml
    };
    let mut xml = String::from(
        "<?xml version=\"1.0\"?>\n<!DOCTYPE module PUBLIC\n  \
         \"-//Checkstyle//DTD Checkstyle Configuration 1.3//EN\"\n  \
         \"https://checkstyle.org/dtds/configuration_1_3.dtd\">\n\
         <module name=\"Checker\">\n  \
         <property name=\"haltOnException\" value=\"false\"/>\n  \
         <module name=\"SuppressWarningsFilter\"/>\n",
    );
    for check in checks.iter().filter(|check| FILE_CHECKS.contains(check)) {
        xml.push_str(&module(check, "  "));
    }
    xml.push_str(
        "  <module name=\"TreeWalker\">\n    <module name=\"SuppressWarningsHolder\"/>\n    \
         <module name=\"SuppressionCommentFilter\"/>\n",
    );
    for check in checks.iter().filter(|check| !FILE_CHECKS.contains(check)) {
        xml.push_str(&module(check, "    "));
    }
    xml.push_str("  </module>\n</module>\n");
    xml
}

/// A Checkstyle configuration: one written for the run, or one of those
/// that come inside Checkstyle's jar.
#[derive(Clone, Copy)]
enum CheckstyleConfig<'a> {
    Xml(&'a str),
    /// By its resource path, such as `/sun_checks.xml`.
    Bundled(&'a str),
}

fn run_checkstyle(
    sources: &[Source],
    jar: &Path,
    jdk: &Jdk,
    config: CheckstyleConfig<'_>,
) -> Result<Result<Report, String>> {
    let dir = scratch_dir("checkstyle")?;
    let config = match config {
        CheckstyleConfig::Xml(xml) => {
            let file = dir.join("config.xml");
            fs::write(&file, xml)?;
            file
        }
        CheckstyleConfig::Bundled(resource) => PathBuf::from(resource),
    };
    let chunks: Vec<(usize, &[Source])> = sources.chunks(CHUNK).enumerate().collect();
    let reports: Vec<Result<Result<CheckstyleReport, String>>> = chunks
        .par_iter()
        .map(|(i, chunk)| {
            let out = dir.join(format!("report-{i}.xml"));
            // Checkstyle exits non-zero when it finds violations, so the exit
            // status says nothing; the report file is what matters.
            let output = Command::new(&jdk.java)
                .arg("-jar")
                .arg(jar)
                .arg("-c")
                .arg(&config)
                .args(["-f", "xml", "-o"])
                .arg(&out)
                .args(chunk.iter().map(|source| &source.path))
                .output()
                .context("cannot run Checkstyle")?;
            let Ok(xml) = fs::read_to_string(&out) else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Ok(Err(format!(
                    "Checkstyle failed: {}",
                    stderr.lines().next().unwrap_or("?")
                )));
            };
            let mut report = CheckstyleReport::default();
            parse_checkstyle_xml(&xml, &mut report);
            // A file the report says nothing about was not checked.
            for source in chunk.iter() {
                let path = source.path.display().to_string();
                if !report.seen.contains(&path) {
                    report.failed.insert(path);
                }
            }
            Ok(Ok(report))
        })
        .collect();
    fs::remove_dir_all(&dir).ok();

    let index_of: HashMap<String, usize> = sources
        .iter()
        .enumerate()
        .map(|(i, s)| (s.path.display().to_string(), i))
        .collect();
    let mut merged = Report::default();
    for report in reports {
        let report = match report? {
            Ok(report) => report,
            Err(why) => return Ok(Err(why)),
        };
        for (file, check, line) in report.violations {
            if let Some(&i) = index_of.get(&file) {
                merged.violations.push((i, check, line));
            }
        }
        merged
            .failed
            .extend(report.failed.iter().filter_map(|file| index_of.get(file)));
    }
    merged.violations.sort();
    Ok(Ok(merged))
}

/// Share of diagnostics on which WeaveJig and Checkstyle agree by rule and
/// line: matched / (matched + reported by only one of them).
pub fn checkstyle(
    sources: &[Source],
    manifest: &Manifest,
    settings: &Settings<'_>,
) -> Result<Score> {
    const PART: &str = "Linter";
    let reference = manifest.reference("checkstyle")?;
    let name = format!("Checkstyle {}", reference.version);
    let jar = reference.jar_path();
    if !jar.is_file() {
        return Ok(Score::unavailable(
            PART,
            name,
            "not downloaded; run `cargo xtask fetch references`",
        ));
    }
    let jdk = match java::locate(reference.min_jdk) {
        Ok(jdk) => jdk,
        Err(why) => return Ok(Score::unavailable(PART, name, why)),
    };

    let rule_for = mirrored(|meta| meta.checkstyle);
    let mut checks: Vec<&str> = rule_for.keys().copied().collect();
    checks.sort_unstable();
    let config = checkstyle_config(&checks);
    let key = cache_key(&name, &config, sources);
    let report = match cached(&key, settings, || {
        run_checkstyle(sources, &jar, &jdk, CheckstyleConfig::Xml(&config))
    })? {
        Ok(report) => report,
        Err(why) => return Ok(Score::unavailable(PART, name, why)),
    };
    let rules = rule_set(&rule_for, SuppressionNames::CHECKSTYLE)?;
    Ok(compare(
        sources,
        name,
        &report,
        &rule_for,
        &rules,
        LineMatch::Start,
        settings,
    ))
}

/// Checks about layout: indentation, whitespace, and where braces and line
/// breaks go. The formatter settles all of that, so the linter has no rules
/// for them, and they are left out of the coverage it is measured on.
const LAYOUT_CHECKS: [&str; 26] = [
    "AnnotationLocation",
    "ArrayBracketNoWhitespace",
    "CommentsIndentation",
    "CustomImportOrder",
    "EmptyForIteratorPad",
    "EmptyLineSeparator",
    "GenericWhitespace",
    "GoogleRightCurly",
    "ImportOrder",
    "Indentation",
    "LeftCurly",
    "MethodParamPad",
    "NoLineWrap",
    "NoWhitespaceAfter",
    "NoWhitespaceBefore",
    "NoWhitespaceBeforeCaseDefaultColon",
    "OperatorWrap",
    "ParenPad",
    "RightCurly",
    "SeparatorWrap",
    "SingleSpaceSeparator",
    "TextBlockGoogleStyleFormatting",
    "TypecastParenPad",
    "WhitespaceAfter",
    "WhitespaceAround",
    "WhitespaceBeforeEmptyBody",
];

/// How much of what one of Checkstyle's own configurations reports comes
/// from checks WeaveJig has a rule for.
///
/// The other linter score says how well the rules WeaveJig has agree with
/// the checks they mirror. This one says how many rules it has: of the
/// diagnostics a team running `config` would see, the share WeaveJig could
/// report at all. It counts checks, not lines, since the configuration sets
/// options that WeaveJig's rules are not run with here.
pub fn coverage(
    sources: &[Source],
    manifest: &Manifest,
    settings: &Settings<'_>,
    config: &'static str,
) -> Result<Score> {
    const PART: &str = "Coverage";
    let reference = manifest.reference("checkstyle")?;
    let name = format!("Checkstyle {config}");
    let jar = reference.jar_path();
    if !jar.is_file() {
        return Ok(Score::unavailable(
            PART,
            name,
            "not downloaded; run `cargo xtask fetch references`",
        ));
    }
    let jdk = match java::locate(reference.min_jdk) {
        Ok(jdk) => jdk,
        Err(why) => return Ok(Score::unavailable(PART, name, why)),
    };
    let resource = format!("/{config}.xml");
    let tool = format!("Checkstyle {}", reference.version);
    let key = cache_key(&tool, &resource, sources);
    let report = match cached(&key, settings, || {
        run_checkstyle(sources, &jar, &jdk, CheckstyleConfig::Bundled(&resource))
    })? {
        Ok(report) => report,
        Err(why) => return Ok(Score::unavailable(PART, name, why)),
    };

    let mirrored_checks = mirrored(|meta| meta.checkstyle);
    let (mut mirrored, mut layout, mut other) = (0, 0, 0);
    let mut missing: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, check, _) in &report.violations {
        if mirrored_checks.contains_key(check.as_str()) {
            mirrored += 1;
        } else if LAYOUT_CHECKS.contains(&check.as_str()) {
            layout += 1;
        } else {
            other += 1;
            *missing.entry(check).or_default() += 1;
        }
    }
    let considered = mirrored + other;
    let detail = format!(
        "{mirrored}/{considered} diagnostics are from checks WeaveJig mirrors \
         ({layout} more are about layout, which is the formatter's)"
    );
    // The checks most worth adding next.
    let mut missing: Vec<(&str, usize)> = missing.into_iter().collect();
    missing.sort_by_key(|(check, count)| (std::cmp::Reverse(*count), *check));
    let mismatches = missing
        .iter()
        .map(|(check, count)| format!("{check}: {count} diagnostics, no rule"))
        .collect();
    Ok(Score::measured(
        PART, name, mirrored, considered, detail, mismatches,
    ))
}

// --- PMD ----------------------------------------------------------------------

/// Properties that make a PMD rule behave as the WeaveJig rule that mirrors
/// it does by default, where the two tools' defaults differ.
const PMD_PROPERTIES: [(&str, &[(&str, &str)]); 0] = [];

/// A PMD ruleset enabling exactly the rules WeaveJig mirrors.
fn pmd_ruleset(rules: &[PmdRule]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\"?>\n\
         <ruleset name=\"weavejig\"\n    \
         xmlns=\"http://pmd.sourceforge.net/ruleset/2.0.0\"\n    \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"\n    \
         xsi:schemaLocation=\"http://pmd.sourceforge.net/ruleset/2.0.0 \
         https://pmd.sourceforge.io/ruleset_2_0_0.xsd\">\n  \
         <description>The PMD rules WeaveJig mirrors</description>\n",
    );
    for rule in rules {
        let reference = format!("category/java/{}.xml/{}", rule.category, rule.name);
        let properties = PMD_PROPERTIES
            .iter()
            .find(|(name, _)| *name == rule.name)
            .map_or(&[][..], |(_, properties)| properties);
        if properties.is_empty() {
            xml.push_str(&format!("  <rule ref=\"{reference}\"/>\n"));
            continue;
        }
        xml.push_str(&format!("  <rule ref=\"{reference}\">\n    <properties>\n"));
        for (name, value) in properties {
            xml.push_str(&format!(
                "      <property name=\"{name}\" value=\"{}\"/>\n",
                xml_escape(value)
            ));
        }
        xml.push_str("    </properties>\n  </rule>\n");
    }
    xml.push_str("</ruleset>\n");
    xml
}

/// Reads PMD's XML report (`-f xml`): violations by file, rule and line, and
/// the files it could not process.
fn parse_pmd_xml(xml: &str) -> (Vec<(String, String, u32)>, HashSet<String>) {
    let mut violations = Vec::new();
    let mut failed = HashSet::new();
    let mut file = String::new();
    for tag in xml.split('<').skip(1) {
        if tag.starts_with("file ") {
            file = xml_attribute(tag, "name").unwrap_or_default();
        } else if tag.starts_with("violation ") {
            let rule = xml_attribute(tag, "rule");
            let line = xml_attribute(tag, "beginline").and_then(|line| line.parse().ok());
            if let (Some(rule), Some(line)) = (rule, line) {
                violations.push((file.clone(), rule, line));
            }
        } else if tag.starts_with("error ") {
            failed.extend(xml_attribute(tag, "filename"));
        }
    }
    (violations, failed)
}

fn run_pmd(
    sources: &[Source],
    lib: &Path,
    jdk: &Jdk,
    ruleset_xml: &str,
) -> Result<Result<Report, String>> {
    let dir = scratch_dir("pmd")?;
    let ruleset = dir.join("ruleset.xml");
    fs::write(&ruleset, ruleset_xml)?;
    let list = dir.join("files.txt");
    let paths: Vec<String> = sources
        .iter()
        .map(|source| source.path.display().to_string())
        .collect();
    fs::write(&list, paths.join("\n") + "\n")?;
    let out = dir.join("report.xml");
    // PMD exits non-zero when it finds violations, so the exit status says
    // nothing; the report file is what matters.
    let output = Command::new(&jdk.java)
        .arg("-cp")
        .arg(lib.join("*"))
        .arg("net.sourceforge.pmd.cli.PmdCli")
        .args(["check", "--no-cache", "--no-progress", "-f", "xml", "-R"])
        .arg(&ruleset)
        .arg("--file-list")
        .arg(&list)
        .arg("-r")
        .arg(&out)
        .output()
        .context("cannot run PMD")?;
    let xml = fs::read_to_string(&out);
    fs::remove_dir_all(&dir).ok();
    let Ok(xml) = xml else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let why = stderr
            .lines()
            .find(|line| line.contains("ERROR"))
            .or_else(|| stderr.lines().next())
            .unwrap_or("?");
        return Ok(Err(format!("PMD failed: {why}")));
    };
    let (violations, failed) = parse_pmd_xml(&xml);
    let index_of: HashMap<&str, usize> = paths
        .iter()
        .enumerate()
        .map(|(i, path)| (path.as_str(), i))
        .collect();
    let mut report = Report::default();
    for (file, rule, line) in violations {
        if let Some(&i) = index_of.get(file.as_str()) {
            report.violations.push((i, rule, line));
        }
    }
    report
        .failed
        .extend(failed.iter().filter_map(|file| index_of.get(file.as_str())));
    report.violations.sort();
    Ok(Ok(report))
}

/// The directory of PMD's jars inside its unpacked distribution.
fn pmd_lib(unpacked: &Path) -> Option<PathBuf> {
    fs::read_dir(unpacked)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path().join("lib"))
        .find(|lib| lib.is_dir())
}

/// Share of diagnostics on which WeaveJig and PMD agree by rule and line.
pub fn pmd(sources: &[Source], manifest: &Manifest, settings: &Settings<'_>) -> Result<Score> {
    const PART: &str = "Linter";
    let reference = manifest.reference("pmd")?;
    let name = format!("PMD {}", reference.version);
    let Some(lib) = pmd_lib(&reference.unpacked_dir()) else {
        return Ok(Score::unavailable(
            PART,
            name,
            "not downloaded; run `cargo xtask fetch references`",
        ));
    };
    let jdk = match java::locate(reference.min_jdk) {
        Ok(jdk) => jdk,
        Err(why) => return Ok(Score::unavailable(PART, name, why)),
    };

    let rule_for = mirrored(|meta| meta.pmd.map(|rule| rule.name));
    if rule_for.is_empty() {
        return Ok(Score::unavailable(
            PART,
            name,
            "no rule mirrors a PMD rule yet",
        ));
    }
    let mut rules: Vec<PmdRule> = rule_for.values().filter_map(|meta| meta.pmd).collect();
    rules.sort_unstable_by_key(|rule| (rule.category, rule.name));
    let ruleset = pmd_ruleset(&rules);
    let key = cache_key(&name, &ruleset, sources);
    let report = match cached(&key, settings, || run_pmd(sources, &lib, &jdk, &ruleset))? {
        Ok(report) => report,
        Err(why) => return Ok(Score::unavailable(PART, name, why)),
    };
    let rules = rule_set(&rule_for, SuppressionNames::PMD)?;
    Ok(compare(
        sources,
        name,
        &report,
        &rule_for,
        &rules,
        LineMatch::Within,
        settings,
    ))
}

/// The per-rule table printed under the scores.
pub fn render_rules(score: &Score) -> String {
    if score.rules.is_empty() {
        return String::new();
    }
    let id_width = score
        .rules
        .iter()
        .map(|r| r.rule.len())
        .max()
        .unwrap_or(0)
        .max(4);
    let name_width = score
        .rules
        .iter()
        .map(|r| r.mirrors.len())
        .max()
        .unwrap_or(0)
        .max(7);
    let mut out = format!(
        "\nLinter rules against {}:\n  {:<id_width$}  {:<name_width$}  {:>7}  {:>7}  {:>6}  {:>6}\n",
        score.reference, "Rule", "Mirrors", "Matched", "Missed", "Extra", "Score"
    );
    for rule in &score.rules {
        let percent = match rule.percent() {
            Some(percent) => format!("{percent:.1}%"),
            None => "-".to_string(),
        };
        out.push_str(&format!(
            "  {:<id_width$}  {:<name_width$}  {:>7}  {:>7}  {:>6}  {percent:>6}\n",
            rule.rule, rule.mirrors, rule.matched, rule.missed, rule.extra
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkstyle_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<checkstyle version="14.4.0">
<file name="/src/A.java">
<error line="6" column="33" severity="error" message="Should use uppercase &apos;L&apos;." source="com.puppycrawl.tools.checkstyle.checks.UpperEllCheck"/>
<error line="11" column="5" severity="error" message="&apos;if&apos; construct must use &apos;{}&apos;s." source="com.puppycrawl.tools.checkstyle.checks.blocks.NeedBracesCheck"/>
<error line="20" severity="error" message="Javadoc comment at column 8 has parse error. Details: no viable alternative" source="com.puppycrawl.tools.checkstyle.checks.imports.UnusedImportsCheck"/>
</file>
<file name="/src/B&amp;C.java">
<error line="1" severity="error" message="IllegalStateException occurred while parsing file /src/B&amp;C.java." source="com.puppycrawl.tools.checkstyle.Checker"/>
</file>
<file name="/src/D.java">
</file>
</checkstyle>
"#;
        let mut report = CheckstyleReport::default();
        parse_checkstyle_xml(xml, &mut report);
        assert_eq!(
            report.violations,
            [
                ("/src/A.java".to_string(), "UpperEll".to_string(), 6),
                ("/src/A.java".to_string(), "NeedBraces".to_string(), 11),
            ]
        );
        assert_eq!(report.failed, HashSet::from(["/src/B&C.java".to_string()]));
        assert_eq!(report.seen.len(), 3);
    }

    #[test]
    fn checkstyle_config_places_checks() {
        let xml = checkstyle_config(&["LineLength", "NeedBraces", "UpperEll"]);
        let walker = xml.find("<module name=\"TreeWalker\">").unwrap();
        // File checks are children of `Checker`, with their properties.
        let line_length = xml.find("<module name=\"LineLength\">").unwrap();
        assert!(line_length < walker);
        assert!(xml.contains("<property name=\"max\" value=\"100\"/>"));
        assert!(xml.find("<module name=\"NeedBraces\"/>").unwrap() > walker);
        assert!(xml.find("<module name=\"UpperEll\"/>").unwrap() > walker);
        assert!(xml.contains("<module name=\"SuppressWarningsHolder\"/>"));
    }

    #[test]
    fn layout_checks_have_no_rule() {
        // A check is either the formatter's or the linter's, not both.
        let mirrored = mirrored(|meta| meta.checkstyle);
        for check in LAYOUT_CHECKS {
            assert!(!mirrored.contains_key(check), "{check}");
        }
        assert!(LAYOUT_CHECKS.is_sorted());
    }

    #[test]
    fn every_mirrored_rule_names_a_distinct_check() {
        for name_of in [
            (|meta: &RuleMeta| meta.checkstyle) as fn(&RuleMeta) -> Option<&'static str>,
            |meta: &RuleMeta| meta.pmd.map(|rule| rule.name),
        ] {
            let checks: Vec<_> = all_rules()
                .iter()
                .filter_map(|r| name_of(r.meta()))
                .collect();
            let distinct: HashSet<_> = checks.iter().collect();
            assert_eq!(checks.len(), distinct.len());
        }
    }

    #[test]
    fn pmd_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<pmd xmlns="http://pmd.sourceforge.net/report/2.0.0" version="7.29.0">
<file name="/src/A.java">
<violation beginline="53" endline="54" begincolumn="14" endcolumn="29" rule="ControlStatementBraces" ruleset="Code Style" priority="3">
This statement should have braces
</violation>
</file>
<error filename="/src/B&amp;C.java" msg="ParseException: Parse exception in file">
<![CDATA[stack trace]]>
</error>
<suppressedviolation filename="/src/A.java" suppressiontype="nopmd" msg="x" usermsg=""/>
</pmd>
"#;
        let (violations, failed) = parse_pmd_xml(xml);
        assert_eq!(
            violations,
            [(
                "/src/A.java".to_string(),
                "ControlStatementBraces".to_string(),
                53
            )]
        );
        assert_eq!(failed, HashSet::from(["/src/B&C.java".to_string()]));
    }

    #[test]
    fn pmd_ruleset_names_rules_by_category() {
        let xml = pmd_ruleset(&[PmdRule {
            category: "codestyle",
            name: "ControlStatementBraces",
        }]);
        assert!(xml.contains("<rule ref=\"category/java/codestyle.xml/ControlStatementBraces\"/>"));
    }

    #[test]
    fn cache_round_trip() {
        let report = Report {
            violations: vec![
                (0, "NeedBraces".to_string(), 7),
                (3, "UpperEll".to_string(), 1),
            ],
            failed: BTreeSet::from([2]),
        };
        let path =
            std::env::temp_dir().join(format!("weavejig-cache-test-{}.tsv", std::process::id()));
        write_cache(&path, &report).unwrap();
        assert_eq!(read_cache(&path), Some(report));
        fs::write(&path, "nonsense\n").unwrap();
        assert_eq!(read_cache(&path), None);
        fs::remove_file(&path).ok();
    }

    #[test]
    fn cache_key_depends_on_everything() {
        let source = |path: &str, text: &str| Source {
            path: PathBuf::from(path),
            text: text.to_string(),
        };
        let base = cache_key("tool 1", "config", &[source("A.java", "class A {}")]);
        assert_eq!(
            base,
            cache_key("tool 1", "config", &[source("A.java", "class A {}")])
        );
        assert_ne!(
            base,
            cache_key("tool 2", "config", &[source("A.java", "class A {}")])
        );
        assert_ne!(
            base,
            cache_key("tool 1", "other", &[source("A.java", "class A {}")])
        );
        assert_ne!(
            base,
            cache_key("tool 1", "config", &[source("B.java", "class A {}")])
        );
        assert_ne!(
            base,
            cache_key("tool 1", "config", &[source("A.java", "class B {}")])
        );
    }

    #[test]
    fn comparison_counts_by_rule() {
        let sources = [Source {
            path: PathBuf::from("A.java"),
            text: "class A {\n  long a = 1l;\n  long b = 2l;\n}\n".to_string(),
        }];
        let rule_for = mirrored(|meta| meta.checkstyle);
        let rules = rule_set(&rule_for, SuppressionNames::CHECKSTYLE).unwrap();
        let report = Report {
            violations: vec![
                (0, "UpperEll".to_string(), 2),
                (0, "NeedBraces".to_string(), 9),
            ],
            failed: BTreeSet::new(),
        };
        let settings = Settings {
            use_cache: false,
            only_rule: Some("upper-ell"),
        };
        let compared = |line_match| {
            compare(
                &sources,
                "ref".into(),
                &report,
                &rule_for,
                &rules,
                line_match,
                &settings,
            )
        };
        let score = compared(LineMatch::Start);
        let rule = |id: &str| score.rules.iter().find(|r| r.rule == id).unwrap().clone();
        assert_eq!((rule("upper-ell").matched, rule("upper-ell").extra), (1, 1));
        assert_eq!(rule("need-braces").missed, 1);
        // Only the selected rule's disagreements are listed.
        assert_eq!(
            score.mismatches,
            ["A.java:3: upper-ell: only WeaveJig reports this"]
        );
        assert_eq!(compared(LineMatch::Within).mismatches, score.mismatches);
    }

    #[test]
    fn a_reference_line_may_be_inside_a_diagnostic() {
        // The `if` without braces is on line 3, its statement on line 4.
        let sources = [Source {
            path: PathBuf::from("A.java"),
            text: "class A {\n  void m(int n) {\n    if (n > 0)\n      n++;\n  }\n}\n".to_string(),
        }];
        let rule_for = mirrored(|meta| meta.checkstyle);
        let rules = rule_set(&rule_for, SuppressionNames::NONE).unwrap();
        let report = Report {
            violations: vec![(0, "NeedBraces".to_string(), 4)],
            failed: BTreeSet::new(),
        };
        let settings = Settings {
            use_cache: false,
            only_rule: Some("need-braces"),
        };
        let matched = |line_match| {
            let score = compare(
                &sources,
                "ref".into(),
                &report,
                &rule_for,
                &rules,
                line_match,
                &settings,
            );
            score
                .rules
                .iter()
                .find(|r| r.rule == "need-braces")
                .unwrap()
                .matched
        };
        assert_eq!(matched(LineMatch::Start), 0);
        assert_eq!(matched(LineMatch::Within), 1);
    }
}
