//! `cargo xtask score`: measures each part of WeaveJig against its reference
//! tool over a corpus of Java files.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use serde::Serialize;
use weavejig_formatter::{FormatOptions, format};
use weavejig_syntax::parse;

use crate::format_diff::{self, Kind};
use crate::java::{self, Jdk};
use crate::lint_score::RuleScore;
use crate::manifest::{Manifest, cache_dir, repo_root};
use crate::scramble::scramble;

/// Files per reference-tool invocation, to stay under command-line limits.
pub(crate) const CHUNK: usize = 400;

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

/// How many files differ from the reference in one way.
#[derive(Debug, Serialize)]
pub struct KindCount {
    pub kind: Kind,
    pub files: usize,
}

/// The formatter's score on one codebase of a corpus.
#[derive(Debug, Serialize)]
pub struct CodebaseScore {
    pub codebase: String,
    pub identical: usize,
    pub files: usize,
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
    /// For the linter: how each rule compares with the check it mirrors.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<RuleScore>,
    /// For the formatter: the files that differ, counted by kind of
    /// difference.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<KindCount>,
    /// For the formatter: the score of each codebase of the corpus.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub codebases: Vec<CodebaseScore>,
    /// Individual disagreements, for development.
    #[serde(skip)]
    pub mismatches: Vec<String>,
}

impl Score {
    pub(crate) fn measured(
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
            rules: Vec::new(),
            kinds: Vec::new(),
            codebases: Vec::new(),
            mismatches,
        }
    }

    pub(crate) fn unavailable(
        part: &'static str,
        reference: String,
        why: impl Into<String>,
    ) -> Score {
        Score {
            part,
            reference,
            matched: None,
            total: None,
            percent: None,
            detail: why.into(),
            rules: Vec::new(),
            kinds: Vec::new(),
            codebases: Vec::new(),
            mismatches: Vec::new(),
        }
    }
}

/// Paths are shown relative to the repository when they are inside it.
pub(crate) fn display(path: &Path) -> String {
    let root = fs::canonicalize(repo_root()).unwrap_or_else(|_| repo_root());
    path.strip_prefix(&root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// A scratch directory under `.weavejig/tmp`, emptied first.
pub(crate) fn scratch_dir(name: &str) -> Result<PathBuf> {
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

/// What the formatter is scored on.
#[derive(Clone, Copy, PartialEq)]
pub enum FormatInput {
    /// The corpus files as they are.
    AsIs,
    /// The corpus files with their whitespace rewritten by [`scramble`], so
    /// that code already in google-java-format style still has to be
    /// reformatted.
    Scrambled,
}

/// Share of files that WeaveJig formats byte-for-byte like google-java-format.
/// `roots` are the corpus directories, which the score is also broken down by.
pub fn formatter(
    sources: &[Source],
    roots: &[PathBuf],
    manifest: &Manifest,
    input: FormatInput,
) -> Result<Score> {
    const PART: &str = "Formatter";
    let reference = manifest.reference("google-java-format")?;
    let mut name = format!("google-java-format {}", reference.version);
    if input == FormatInput::Scrambled {
        name.push_str(", scrambled");
    }
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

    let scrambled: Vec<Source>;
    let sources = match input {
        FormatInput::AsIs => sources,
        FormatInput::Scrambled => {
            scrambled = sources
                .par_iter()
                .map(|source| Source {
                    path: source.path.clone(),
                    text: scramble(&source.text),
                })
                .collect();
            &scrambled
        }
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
        /// The kind of difference, and the first line that differs.
        Different(Kind, usize),
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
                Ok(actual) => Outcome::Different(
                    format_diff::classify(&expected, &actual),
                    format_diff::first_difference(&expected, &actual),
                ),
                Err(error) => Outcome::Unformattable(error.to_string()),
            }
        })
        .collect();
    fs::remove_dir_all(&dir).ok();

    let (mut identical, mut total, mut unformattable, mut excluded) = (0, 0, 0, 0);
    let mut mismatches = Vec::new();
    let mut kinds: HashMap<Kind, usize> = HashMap::new();
    let roots: Vec<PathBuf> = roots
        .iter()
        .map(|root| fs::canonicalize(root).unwrap_or_else(|_| root.clone()))
        .collect();
    let mut codebases: Vec<CodebaseScore> = roots
        .iter()
        .map(|root| CodebaseScore {
            codebase: root.file_name().map_or_else(
                || root.display().to_string(),
                |name| name.to_string_lossy().into(),
            ),
            identical: 0,
            files: 0,
        })
        .collect();
    for (source, outcome) in sources.iter().zip(outcomes) {
        let codebase = roots
            .iter()
            .position(|root| source.path.starts_with(root))
            .map(|i| &mut codebases[i]);
        match outcome {
            Outcome::Excluded => {
                excluded += 1;
                continue;
            }
            Outcome::Identical => {
                identical += 1;
                if let Some(codebase) = codebase {
                    codebase.identical += 1;
                    codebase.files += 1;
                }
                total += 1;
                continue;
            }
            Outcome::Different(kind, line) => {
                *kinds.entry(kind).or_default() += 1;
                mismatches.push(format!(
                    "{}:{line}: output differs ({})",
                    display(&source.path),
                    kind.label()
                ));
            }
            Outcome::Unformattable(error) => {
                unformattable += 1;
                mismatches.push(format!("{}: {error}", display(&source.path)));
            }
        }
        if let Some(codebase) = codebase {
            codebase.files += 1;
        }
        total += 1;
    }
    let detail = format!(
        "{identical}/{total} files identical ({unformattable} WeaveJig could not format; \
         {excluded} excluded, rejected by the reference)"
    );
    let mut score = Score::measured(PART, name, identical, total, detail, mismatches);
    score.kinds = Kind::ALL
        .into_iter()
        .filter_map(|kind| kinds.get(&kind).map(|&files| KindCount { kind, files }))
        .collect();
    score
        .kinds
        .sort_by_key(|count| std::cmp::Reverse(count.files));
    // One codebase's score is the corpus's.
    if codebases.len() > 1 {
        score.codebases = codebases;
    }
    Ok(score)
}

/// The formatter's breakdowns: how many files differ in each way, and the
/// score of each codebase. Both are totals, fit to print for a held-out set.
pub fn render_formatter(score: &Score) -> String {
    let mut out = String::new();
    if !score.kinds.is_empty() {
        let width = score
            .kinds
            .iter()
            .map(|count| count.kind.label().len())
            .max()
            .unwrap_or(0)
            .max(4);
        out.push_str(&format!(
            "\nFormatter differences from {}, by kind:\n  {:<width$}  {:>5}\n",
            score.reference, "Kind", "Files"
        ));
        for count in &score.kinds {
            out.push_str(&format!(
                "  {:<width$}  {:>5}\n",
                count.kind.label(),
                count.files
            ));
        }
    }
    if !score.codebases.is_empty() {
        let width = score
            .codebases
            .iter()
            .map(|codebase| codebase.codebase.len())
            .max()
            .unwrap_or(0)
            .max(8);
        out.push_str(&format!(
            "\nFormatter against {}, by codebase:\n  {:<width$}  {:>9}  {:>5}  {:>6}\n",
            score.reference, "Codebase", "Identical", "Files", "Score"
        ));
        for codebase in &score.codebases {
            let percent = if codebase.files > 0 {
                format!(
                    "{:.1}%",
                    100.0 * codebase.identical as f64 / codebase.files as f64
                )
            } else {
                "-".to_string()
            };
            out.push_str(&format!(
                "  {:<width$}  {:>9}  {:>5}  {percent:>6}\n",
                codebase.codebase, codebase.identical, codebase.files
            ));
        }
    }
    out
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
            Score::unavailable("Linter", "PMD 7.29.0".into(), "not downloaded"),
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
        assert!(table.contains("\n           PMD 7.29.0 "), "{table}");
    }
}
