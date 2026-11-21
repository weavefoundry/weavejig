//! Development tasks for WeaveJig. Run with `cargo xtask <command>`.

mod fetch;
mod java;
mod manifest;
mod score;

use std::path::PathBuf;

use anyhow::{Result, bail};
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
    },
    /// Download what scoring needs into `.weavejig/`
    Fetch {
        what: FetchTarget,

        /// Which corpus set to download
        #[arg(long, default_value = "dev")]
        set: String,
    },
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
        } => score(&manifest, &set, corpus, json, show),
    }
}

fn score(
    manifest: &Manifest,
    set: &str,
    corpus: Vec<PathBuf>,
    json: bool,
    show: usize,
) -> Result<()> {
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

    let scores = [
        score::parser(&sources, &jdk)?,
        score::formatter(&sources, manifest)?,
        score::linter(&sources, manifest)?,
        score::pmd(),
    ];

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
        if score.mismatches.is_empty() || show == 0 {
            continue;
        }
        if blind {
            println!(
                "\n{}: {} disagreements (not listed for held-out sets)",
                score.part,
                score.mismatches.len()
            );
            continue;
        }
        println!("\n{} disagreements with {}:", score.part, score.reference);
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
