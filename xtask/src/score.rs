//! `cargo xtask score`: measures each part of WeaveJig against its reference
//! tool over a corpus of Java files.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use serde::Serialize;
use weavejig_formatter::{FormatOptions, format};
use weavejig_linter::{RuleSet, all_rules, lint_tree};
use weavejig_syntax::{LineIndex, parse};

use crate::java::{self, Jdk};
use crate::manifest::{Manifest, cache_dir, repo_root};

/// Files per reference-tool invocation, to stay under command-line limits.
const CHUNK: usize = 400;

pub struct Source {
    pub path: PathBuf,
    pub text: String,
}

/// Reads every `.java` file under `dirs`. Returns the sources and how many
/// files were skipped for not being UTF-8.
pub fn load_sources(dirs: &[PathBuf]) -> Result<(Vec<Source>, usize)> {
    let mut paths = Vec::new();
    for dir in dirs {
        if !dir.is_dir() {
            bail!("corpus directory {} does not exist", dir.display());
        }
        // Corpus checkouts are scored whole: no `.gitignore` filtering.
        for entry in ignore::WalkBuilder::new(dir)
            .standard_filters(false)
            .build()
        {
            let entry = entry?;
            let path = entry.path();
            let in_git = path.components().any(|c| c.as_os_str() == ".git");
            if !in_git && path.is_file() && path.extension().is_some_and(|ext| ext == "java") {
                paths.push(fs::canonicalize(path)?);
            }
        }
    }
    paths.sort();
    paths.dedup();
    let mut skipped = 0;
    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        match fs::read_to_string(&path) {
            Ok(text) => sources.push(Source { path, text }),
            Err(_) => skipped += 1,
        }
    }
    Ok((sources, skipped))
}

#[derive(Debug, Serialize)]
pub struct Score {
    pub part: &'static str,
    pub reference: String,
    /// Units (files or diagnostics) on which WeaveJig matches the reference,
    /// or `None` if the score could not be measured.
    pub matched: Option<usize>,
    pub total: Option<usize>,
    pub percent: Option<f64>,
    pub detail: String,
    /// Individual disagreements, for development.
    #[serde(skip)]
    pub mismatches: Vec<String>,
}

impl Score {
    fn measured(
        part: &'static str,
        reference: String,
        matched: usize,
        total: usize,
        detail: String,
        mismatches: Vec<String>,
    ) -> Score {
        let percent = (total > 0).then(|| 100.0 * matched as f64 / total as f64);
        Score {
            part,
            reference,
            matched: Some(matched),
            total: Some(total),
            percent,
            detail,
            mismatches,
        }
    }

    fn unavailable(part: &'static str, reference: String, why: impl Into<String>) -> Score {
        Score {
            part,
            reference,
            matched: None,
            total: None,
            percent: None,
            detail: why.into(),
            mismatches: Vec::new(),
        }
    }
}

/// Paths are shown relative to the repository when they are inside it.
fn display(path: &Path) -> String {
    let root = fs::canonicalize(repo_root()).unwrap_or_else(|_| repo_root());
    path.strip_prefix(&root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// A scratch directory under `.weavejig/tmp`, emptied first.
fn scratch_dir(name: &str) -> Result<PathBuf> {
    let dir = cache_dir()
        .join("tmp")
        .join(format!("{name}-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    fs::canonicalize(&dir).with_context(|| format!("cannot resolve {}", dir.display()))
}

// --- Parser -----------------------------------------------------------------

/// Parses the output of `reference/JavacParse.java`.
fn parse_javac_output(output: &str) -> (String, HashMap<String, bool>) {
    let mut version = String::from("unknown");
    let mut verdicts = HashMap::new();
    for line in output.lines() {
        match line.split_once('\t') {
            Some(("version", value)) => version = value.to_string(),
            Some(("accept", path)) => {
                verdicts.insert(path.to_string(), true);
            }
            Some(("reject", path)) => {
                verdicts.insert(path.to_string(), false);
            }
            _ => {}
        }
    }
    (version, verdicts)
}

/// Share of files that WeaveJig and `javac` both accept or both reject.
pub fn parser(sources: &[Source], jdk: &Result<Jdk, String>) -> Result<Score> {
    const PART: &str = "Parser";
    let jdk = match jdk {
        Ok(jdk) => jdk,
        Err(why) => return Ok(Score::unavailable(PART, "javac".into(), why.clone())),
    };
    // The helper reads paths from a file rather than a pipe, so neither side
    // can block on the other.
    let dir = scratch_dir("javac")?;
    let list = dir.join("paths.txt");
    let paths: Vec<String> = sources
        .iter()
        .map(|s| s.path.display().to_string())
        .collect();
    fs::write(&list, paths.join("\n") + "\n")?;
    let output = Command::new(&jdk.java)
        .arg(repo_root().join("reference").join("JavacParse.java"))
        .stdin(Stdio::from(File::open(&list)?))
        .output()
        .context("cannot run the javac reference parser")?;
    fs::remove_dir_all(&dir).ok();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let why = format!(
            "reference parser failed: {}",
            stderr.lines().next().unwrap_or("?")
        );
        return Ok(Score::unavailable(
            PART,
            format!("javac {}", jdk.version),
            why,
        ));
    }
    let (version, verdicts) = parse_javac_output(&String::from_utf8_lossy(&output.stdout));

    let ours: Vec<bool> = sources
        .par_iter()
        .map(|source| parse(&source.text).is_ok())
        .collect();
    let (mut agree, mut total, mut only_ours, mut only_javac) = (0, 0, 0, 0);
    let mut mismatches = Vec::new();
    for (source, ours) in sources.iter().zip(ours) {
        let Some(&javac) = verdicts.get(&source.path.display().to_string()) else {
            continue;
        };
        total += 1;
        match (ours, javac) {
            (true, false) => {
                only_ours += 1;
                mismatches.push(format!(
                    "{}: accepted, but javac rejects it",
                    display(&source.path)
                ));
            }
            (false, true) => {
                only_javac += 1;
                mismatches.push(format!(
                    "{}: rejected, but javac accepts it",
                    display(&source.path)
                ));
            }
            _ => agree += 1,
        }
    }
    let detail = format!(
        "{agree}/{total} files agree ({only_javac} wrongly rejected, {only_ours} wrongly accepted)"
    );
    Ok(Score::measured(
        PART,
        format!("javac {version}"),
        agree,
        total,
        detail,
        mismatches,
    ))
}

// --- Formatter --------------------------------------------------------------

/// The files google-java-format reported an error for, from its standard
/// error. Lines look like `/path/File.java:12:3: error: ...`.
fn gjf_failures(stderr: &str) -> HashSet<String> {
    stderr
        .lines()
        .filter_map(|line| {
            let (path, rest) = line.split_once(".java:")?;
            rest.contains("error").then(|| format!("{path}.java"))
        })
        .collect()
}

/// Share of files that WeaveJig formats byte-for-byte like google-java-format.
pub fn formatter(sources: &[Source], manifest: &Manifest) -> Result<Score> {
    const PART: &str = "Formatter";
    let reference = manifest.reference("google-java-format")?;
    let name = format!("google-java-format {}", reference.version);
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

    // google-java-format rewrites copies of the files in place.
    let dir = scratch_dir("gjf")?;
    let mut copies = Vec::with_capacity(sources.len());
    for (i, source) in sources.iter().enumerate() {
        let copy = dir
            .join(i.to_string())
            .join(source.path.file_name().unwrap_or_default());
        fs::create_dir_all(copy.parent().context("copy has a parent")?)?;
        fs::write(&copy, &source.text)?;
        copies.push(copy);
    }
    let mut failed = HashSet::new();
    for chunk in copies.chunks(CHUNK) {
        let output = Command::new(&jdk.java)
            .arg("-jar")
            .arg(&jar)
            .arg("--replace")
            .args(chunk)
            .output()
            .context("cannot run google-java-format")?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        let failures = gjf_failures(&stderr);
        if !output.status.success() && failures.is_empty() {
            fs::remove_dir_all(&dir).ok();
            let why = format!(
                "google-java-format failed: {}",
                stderr.lines().next().unwrap_or("?")
            );
            return Ok(Score::unavailable(PART, name, why));
        }
        failed.extend(failures);
    }

    enum Outcome {
        Identical,
        Different,
        Unformattable(String),
        Excluded,
    }
    let outcomes: Vec<Outcome> = sources
        .par_iter()
        .zip(&copies)
        .map(|(source, copy)| {
            if failed.contains(&copy.display().to_string()) {
                return Outcome::Excluded;
            }
            let Ok(expected) = fs::read_to_string(copy) else {
                return Outcome::Excluded;
            };
            match format(&source.text, &FormatOptions::default()) {
                Ok(actual) if actual == expected => Outcome::Identical,
                Ok(_) => Outcome::Different,
                Err(error) => Outcome::Unformattable(error.to_string()),
            }
        })
        .collect();
    fs::remove_dir_all(&dir).ok();

    let (mut identical, mut total, mut unformattable, mut excluded) = (0, 0, 0, 0);
    let mut mismatches = Vec::new();
    for (source, outcome) in sources.iter().zip(outcomes) {
        match outcome {
            Outcome::Excluded => {
                excluded += 1;
                continue;
            }
            Outcome::Identical => identical += 1,
            Outcome::Different => {
                mismatches.push(format!("{}: output differs", display(&source.path)))
            }
            Outcome::Unformattable(error) => {
                unformattable += 1;
                mismatches.push(format!("{}: {error}", display(&source.path)));
            }
        }
        total += 1;
    }
    let detail = format!(
        "{identical}/{total} files identical ({unformattable} WeaveJig could not format; \
         {excluded} excluded, rejected by the reference)"
    );
    Ok(Score::measured(
        PART, name, identical, total, detail, mismatches,
    ))
}

// --- Linter -----------------------------------------------------------------

fn xml_unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
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
}

/// Reads Checkstyle's XML report (`-f xml`).
fn parse_checkstyle_xml(xml: &str, report: &mut CheckstyleReport) {
    let mut file = String::new();
    for tag in xml.split('<').skip(1) {
        if tag.starts_with("file ") {
            file = xml_attribute(tag, "name").unwrap_or_default();
        } else if tag.starts_with("exception") {
            report.failed.insert(file.clone());
        } else if tag.starts_with("error ") {
            let source = xml_attribute(tag, "source").unwrap_or_default();
            let check = source
                .rsplit('.')
                .next()
                .and_then(|name| name.strip_suffix("Check"));
            let line = xml_attribute(tag, "line").and_then(|line| line.parse().ok());
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

/// A Checkstyle configuration enabling exactly the checks WeaveJig mirrors,
/// with their default options.
fn checkstyle_config(checks: &[&str]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\"?>\n<!DOCTYPE module PUBLIC\n  \
         \"-//Checkstyle//DTD Checkstyle Configuration 1.3//EN\"\n  \
         \"https://checkstyle.org/dtds/configuration_1_3.dtd\">\n\
         <module name=\"Checker\">\n  <module name=\"TreeWalker\">\n",
    );
    for check in checks {
        xml.push_str(&format!("    <module name=\"{check}\"/>\n"));
    }
    xml.push_str("  </module>\n</module>\n");
    xml
}

/// How two sets of diagnostics compare.
#[derive(Debug, PartialEq)]
struct Agreement {
    matched: usize,
    /// Reported by the reference only.
    missing: usize,
    /// Reported by WeaveJig only.
    extra: usize,
}

type DiagnosticKey = (usize, &'static str, u32);

fn compare(reference: &BTreeSet<DiagnosticKey>, ours: &BTreeSet<DiagnosticKey>) -> Agreement {
    let matched = reference.intersection(ours).count();
    Agreement {
        matched,
        missing: reference.len() - matched,
        extra: ours.len() - matched,
    }
}

/// Share of diagnostics on which WeaveJig and Checkstyle agree by rule and
/// line: matched / (matched + reported by only one of them).
pub fn linter(sources: &[Source], manifest: &Manifest) -> Result<Score> {
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

    // Checkstyle check name -> WeaveJig rule id.
    let rule_for: HashMap<&str, &'static str> = all_rules()
        .iter()
        .filter_map(|rule| Some((rule.meta().checkstyle?, rule.meta().id)))
        .collect();
    let mut checks: Vec<&str> = rule_for.keys().copied().collect();
    checks.sort_unstable();
    let rules = RuleSet::from_selection(
        Some(
            &rule_for
                .values()
                .map(|id| id.to_string())
                .collect::<Vec<_>>(),
        ),
        &[],
    )?;

    let dir = scratch_dir("checkstyle")?;
    let config = dir.join("config.xml");
    fs::write(&config, checkstyle_config(&checks))?;
    let mut report = CheckstyleReport::default();
    for (i, chunk) in sources.chunks(CHUNK).enumerate() {
        let out = dir.join(format!("report-{i}.xml"));
        // Checkstyle exits non-zero when it finds violations, so the exit
        // status says nothing; the report file is what matters.
        let output = Command::new(&jdk.java)
            .arg("-jar")
            .arg(&jar)
            .arg("-c")
            .arg(&config)
            .args(["-f", "xml", "-o"])
            .arg(&out)
            .args(chunk.iter().map(|source| &source.path))
            .output()
            .context("cannot run Checkstyle")?;
        let Ok(xml) = fs::read_to_string(&out) else {
            fs::remove_dir_all(&dir).ok();
            let stderr = String::from_utf8_lossy(&output.stderr);
            let why = format!(
                "Checkstyle failed: {}",
                stderr.lines().next().unwrap_or("?")
            );
            return Ok(Score::unavailable(PART, name, why));
        };
        parse_checkstyle_xml(&xml, &mut report);
    }
    fs::remove_dir_all(&dir).ok();

    let index_of: HashMap<String, usize> = sources
        .iter()
        .enumerate()
        .map(|(i, s)| (s.path.display().to_string(), i))
        .collect();
    // Only files both tools could process are compared.
    let ours_per_file: Vec<Option<Vec<(&'static str, u32)>>> = sources
        .par_iter()
        .map(|source| {
            let parse = parse(&source.text);
            if !parse.is_ok() || report.failed.contains(&source.path.display().to_string()) {
                return None;
            }
            let lines = LineIndex::new(&source.text);
            let diagnostics = lint_tree(&parse.syntax(), &rules);
            Some(
                diagnostics
                    .iter()
                    .map(|d| (d.rule, lines.line_col(&source.text, d.range.start()).line))
                    .collect(),
            )
        })
        .collect();

    let mut ours = BTreeSet::new();
    for (i, diagnostics) in ours_per_file.iter().enumerate() {
        for (rule, line) in diagnostics.iter().flatten() {
            ours.insert((i, *rule, *line));
        }
    }
    let mut theirs = BTreeSet::new();
    for (file, check, line) in &report.violations {
        let (Some(&i), Some(&rule)) = (index_of.get(file), rule_for.get(check.as_str())) else {
            continue;
        };
        if ours_per_file[i].is_some() {
            theirs.insert((i, rule, *line));
        }
    }

    let agreement = compare(&theirs, &ours);
    let mut mismatches = Vec::new();
    for (i, rule, line) in theirs.difference(&ours) {
        mismatches.push(format!(
            "{}:{line}: {rule}: only Checkstyle reports this",
            display(&sources[*i].path)
        ));
    }
    for (i, rule, line) in ours.difference(&theirs) {
        mismatches.push(format!(
            "{}:{line}: {rule}: only WeaveJig reports this",
            display(&sources[*i].path)
        ));
    }
    mismatches.sort();
    let total = agreement.matched + agreement.missing + agreement.extra;
    let detail = format!(
        "{}/{total} diagnostics match over {} rules ({} missed, {} extra)",
        agreement.matched,
        checks.len(),
        agreement.missing,
        agreement.extra
    );
    Ok(Score::measured(
        PART,
        name,
        agreement.matched,
        total,
        detail,
        mismatches,
    ))
}

/// PMD is a reference for the linter too, but nothing runs it yet.
pub fn pmd() -> Score {
    Score::unavailable("Linter", "PMD".into(), "not wired up yet")
}

// --- Report -----------------------------------------------------------------

pub fn render(label: &str, files: usize, skipped: usize, scores: &[Score]) -> String {
    let mut out = format!("WeaveJig scores: {label} corpus, {files} files");
    if skipped > 0 {
        out.push_str(&format!(" ({skipped} skipped, not UTF-8)"));
    }
    out.push_str("\n\n");
    let reference_width = scores
        .iter()
        .map(|s| s.reference.len())
        .max()
        .unwrap_or(0)
        .max(9);
    out.push_str(&format!(
        "{:<10} {:<reference_width$}  {:>6}  Detail\n",
        "Part", "Reference", "Score"
    ));
    let mut previous_part = "";
    for score in scores {
        let part = if score.part == previous_part {
            ""
        } else {
            score.part
        };
        previous_part = score.part;
        let percent = match score.percent {
            Some(percent) => format!("{percent:.1}%"),
            None => "-".to_string(),
        };
        out.push_str(&format!(
            "{part:<10} {:<reference_width$}  {percent:>6}  {}\n",
            score.reference, score.detail
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn javac_output() {
        let (version, verdicts) =
            parse_javac_output("version\t21 (21.0.4+7)\naccept\t/a/A.java\nreject\t/a/B.java\n");
        assert_eq!(version, "21 (21.0.4+7)");
        assert_eq!(verdicts.get("/a/A.java"), Some(&true));
        assert_eq!(verdicts.get("/a/B.java"), Some(&false));
    }

    #[test]
    fn gjf_stderr() {
        let failures = gjf_failures(
            "/tmp/x/3/Bad.java:5:14: error: illegal start of expression\n\
             /tmp/x/7/Worse.java:1:1: error: class, interface, enum, or record expected\n\
             WARNING: something unrelated\n",
        );
        assert_eq!(failures.len(), 2);
        assert!(failures.contains("/tmp/x/3/Bad.java"));
        assert!(failures.contains("/tmp/x/7/Worse.java"));
    }

    #[test]
    fn checkstyle_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<checkstyle version="14.4.0">
<file name="/src/A.java">
<error line="6" column="33" severity="error" message="Should use uppercase &apos;L&apos;." source="com.puppycrawl.tools.checkstyle.checks.UpperEllCheck"/>
<error line="11" column="5" severity="error" message="&apos;if&apos; construct must use &apos;{}&apos;s." source="com.puppycrawl.tools.checkstyle.checks.blocks.NeedBracesCheck"/>
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
    }

    #[test]
    fn checkstyle_config_lists_checks() {
        let xml = checkstyle_config(&["NeedBraces", "UpperEll"]);
        assert!(xml.contains("<module name=\"TreeWalker\">"));
        assert!(xml.contains("<module name=\"NeedBraces\"/>"));
        assert!(xml.contains("<module name=\"UpperEll\"/>"));
    }

    #[test]
    fn every_mirrored_rule_names_a_distinct_check() {
        let checks: Vec<_> = all_rules()
            .iter()
            .filter_map(|r| r.meta().checkstyle)
            .collect();
        let distinct: HashSet<_> = checks.iter().collect();
        assert_eq!(checks.len(), distinct.len());
    }

    #[test]
    fn agreement() {
        let reference = BTreeSet::from([(0, "a", 1), (0, "a", 2), (1, "b", 5)]);
        let ours = BTreeSet::from([(0, "a", 2), (1, "b", 5), (1, "b", 9), (2, "a", 1)]);
        assert_eq!(
            compare(&reference, &ours),
            Agreement {
                matched: 2,
                missing: 1,
                extra: 2
            }
        );
    }

    #[test]
    fn report_table() {
        let scores = [
            Score::measured(
                "Parser",
                "javac 21".into(),
                9,
                10,
                "9/10 files agree".into(),
                vec![],
            ),
            Score::unavailable("Linter", "Checkstyle 14.4.0".into(), "not downloaded"),
            pmd(),
        ];
        let table = render("smoke", 10, 0, &scores);
        assert!(
            table.contains("Parser     javac 21            90.0%  9/10 files agree"),
            "{table}"
        );
        assert!(
            table.contains("Linter     Checkstyle 14.4.0       -  not downloaded"),
            "{table}"
        );
        // A repeated part name is printed once.
        assert!(table.contains("\n           PMD "), "{table}");
    }
}
