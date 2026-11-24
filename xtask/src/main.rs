//! Development tasks for WeaveJig. Run with `cargo xtask <command>`.

mod fetch;
mod java;
mod lint_score;
mod manifest;
mod score;
mod scramble;

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};

use crate::manifest::{Manifest, repo_root};

/// See `STACK_SIZE` in the `weavejig` binary.
const STACK_SIZE: usize = 32 * 1024 * 1024;

#[derive(Parser)]
#[command(name = "xtask", about = "Development tasks for WeaveJig")]
struct Cli {
    #[command(subcommand)]
    command: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Score the parser, formatter and linter against their reference tools
    Score {
        /// Corpus to score: `smoke` (in the repository), `dev` or `held-out`
        #[arg(long, default_value = "smoke")]
        set: String,

        /// Score these directories instead of a named set
        #[arg(long, value_name = "DIR")]
        corpus: Vec<PathBuf>,

        /// Print the scores as JSON
        #[arg(long)]
        json: bool,

        /// How many disagreements to list for each part
        #[arg(long, default_value_t = 10, value_name = "N")]
        show: usize,

        /// Score only these parts (comma-separated); all of them by default
        #[arg(long, value_enum, value_delimiter = ',', value_name = "PARTS")]
        part: Vec<Part>,

        /// List the linter's disagreements for this rule only
        #[arg(long, value_name = "RULE")]
        rule: Option<String>,

        /// Run the reference linters again instead of reusing their earlier
        /// output for the same checks and files
        #[arg(long)]
        no_cache: bool,
    },
    /// Print a file with its whitespace scrambled as the formatter score does,
    /// to reproduce a disagreement on scrambled input
    Scramble {
        /// A Java file
        file: PathBuf,
    },
    /// Download what scoring needs into `.weavejig/`
    Fetch {
        what: FetchTarget,

        /// Which corpus set to download
        #[arg(long, default_value = "dev")]
        set: String,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Part {
    Parser,
    Formatter,
    Linter,
}

#[derive(Clone, Copy, ValueEnum)]
enum FetchTarget {
    /// The google-java-format and Checkstyle jars
    References,
    /// The open-source codebases of a corpus set
    Corpus,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let worker = std::thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(move || run(cli))?;
    match worker.join() {
        Ok(result) => result,
        Err(_) => bail!("xtask crashed"),
    }
}

fn run(cli: Cli) -> Result<()> {
    rayon::ThreadPoolBuilder::new()
        .stack_size(STACK_SIZE)
        .build_global()
        .ok();
    let manifest = Manifest::load()?;
    match cli.command {
        Task::Fetch {
            what: FetchTarget::References,
            ..
        } => fetch::references(&manifest),
        Task::Fetch {
            what: FetchTarget::Corpus,
            set,
        } => fetch::corpus(&manifest, &set),
        Task::Score {
            set,
            corpus,
            json,
            show,
            part,
            rule,
            no_cache,
        } => {
            let options = ScoreOptions {
                json,
                show,
                parts: part,
                rule,
                use_cache: !no_cache,
            };
            score(&manifest, &set, corpus, &options)
        }
        Task::Scramble { file } => {
            let text = std::fs::read_to_string(&file)
                .with_context(|| format!("cannot read {}", file.display()))?;
            print!("{}", scramble::scramble(&text));
            Ok(())
        }
    }
}

struct ScoreOptions {
    json: bool,
    show: usize,
    /// Empty for every part.
    parts: Vec<Part>,
    rule: Option<String>,
    use_cache: bool,
}

fn score(
    manifest: &Manifest,
    set: &str,
    corpus: Vec<PathBuf>,
    options: &ScoreOptions,
) -> Result<()> {
    let (json, show) = (options.json, options.show);
    let wanted = |part: Part| options.parts.is_empty() || options.parts.contains(&part);
    if let Some(rule) = &options.rule
        && !weavejig_linter::all_rules()
            .iter()
            .any(|known| known.meta().id == rule)
    {
        bail!("unknown lint rule `{rule}`");
    }
    let (label, dirs) = if !corpus.is_empty() {
        ("custom".to_string(), corpus)
    } else if set == "smoke" {
        (
            set.to_string(),
            vec![repo_root().join("corpus").join("smoke")],
        )
    } else {
        let codebases = manifest.codebases(set);
        if codebases.is_empty() {
            bail!("unknown corpus set `{set}`; expected `smoke`, `dev` or `held-out`");
        }
        if let Some(missing) = codebases
            .iter()
            .find(|codebase| !fetch::is_fetched(codebase))
        {
            bail!(
                "`{}` is not downloaded; run `cargo xtask fetch corpus --set {set}`",
                missing.name
            );
        }
        (
            set.to_string(),
            codebases
                .iter()
                .map(|codebase| codebase.checkout_dir())
                .collect(),
        )
    };
    // The held-out set exists to measure, not to debug against.
    let blind = label == "held-out";

    let (sources, skipped) = score::load_sources(&dirs)?;
    if sources.is_empty() {
        bail!("no Java files found in the {label} corpus");
    }
    // One JDK for the parser reference: the newest requirement of the tools,
    // so that all three parts are judged against the same Java version.
    let min_jdk = manifest
        .references
        .values()
        .map(|r| r.min_jdk)
        .max()
        .unwrap_or(17);
    let jdk = java::locate(min_jdk);

    let mut scores = Vec::new();
    if wanted(Part::Parser) {
        scores.push(score::parser(&sources, &jdk)?);
    }
    if wanted(Part::Formatter) {
        scores.push(score::formatter(
            &sources,
            manifest,
            score::FormatInput::AsIs,
        )?);
        scores.push(score::formatter(
            &sources,
            manifest,
            score::FormatInput::Scrambled,
        )?);
    }
    if wanted(Part::Linter) {
        let settings = lint_score::Settings {
            use_cache: options.use_cache,
            only_rule: options.rule.as_deref(),
        };
        scores.push(lint_score::checkstyle(&sources, manifest, &settings)?);
        scores.push(lint_score::pmd(&sources, manifest, &settings)?);
        // Coverage has no single rule's disagreements to list.
        if options.rule.is_none() {
            for config in ["sun_checks", "google_checks"] {
                scores.push(lint_score::coverage(&sources, manifest, &settings, config)?);
            }
        }
    }

    if json {
        let report = serde_json::json!({
            "corpus": label,
            "files": sources.len(),
            "skipped": skipped,
            "scores": scores,
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    print!("{}", score::render(&label, sources.len(), skipped, &scores));
    for score in &scores {
        print!("{}", lint_score::render_rules(score));
    }
    for score in &scores {
        if score.mismatches.is_empty() || show == 0 {
            continue;
        }
        // Which checks have no rule says nothing about the corpus's files.
        if blind && score.part != "Coverage" {
            println!(
                "\n{}: {} disagreements (not listed for held-out sets)",
                score.part,
                score.mismatches.len()
            );
            continue;
        }
        if score.part == "Coverage" {
            println!("\nChecks of {} without a rule:", score.reference);
        } else {
            println!("\n{} disagreements with {}:", score.part, score.reference);
        }
        if score.reference.ends_with(", scrambled") {
            println!("  (reproduce the input with `cargo xtask scramble <file>`)");
        }
        for mismatch in score.mismatches.iter().take(show) {
            println!("  {mismatch}");
        }
        if score.mismatches.len() > show {
            println!(
                "  ... and {} more (raise --show to see them)",
                score.mismatches.len() - show
            );
        }
    }
    Ok(())
}
