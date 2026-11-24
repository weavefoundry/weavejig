//! The `weavejig` command-line interface.

mod config;
mod files;
mod output;

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use rayon::prelude::*;
use weavejig_formatter::{FormatError, FormatOptions, format};
use weavejig_linter::{OptionDefault, RuleSet, Source, all_rules, fix_all, lint};

use crate::config::StyleName;
use crate::output::{Report, Summary, count};

/// Parsing and formatting recurse through nested code, so worker threads get
/// more stack than the default.
const STACK_SIZE: usize = 32 * 1024 * 1024;

/// Nothing to report.
const EXIT_OK: u8 = 0;
/// Problems were found, or files need formatting.
const EXIT_FINDINGS: u8 = 1;
/// WeaveJig could not do what was asked.
const EXIT_ERROR: u8 = 2;

#[derive(Parser)]
#[command(
    name = "weavejig",
    version,
    about = "A fast linter and formatter for Java"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Use this configuration file instead of searching for `weavejig.toml`
    #[arg(long, global = true, value_name = "FILE")]
    config: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Lint Java files
    Check(CheckArgs),
    /// Format Java files
    Format(FormatArgs),
    /// List the lint rules
    Rules(RulesArgs),
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Args)]
struct CheckArgs {
    /// Files or directories to lint; `-` reads standard input
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,

    /// Apply fixes, and report only what could not be fixed
    #[arg(long)]
    fix: bool,

    #[arg(long, value_enum, default_value = "text", value_name = "FORMAT")]
    output_format: OutputFormat,

    /// Run only these rules (comma-separated; `all` for every rule),
    /// overriding the configuration
    #[arg(long, value_delimiter = ',', value_name = "RULES")]
    select: Vec<String>,

    /// Run these rules as well as the selected ones (comma-separated)
    #[arg(long, value_delimiter = ',', value_name = "RULES")]
    extend_select: Vec<String>,

    /// Skip these rules (comma-separated), overriding the configuration
    #[arg(long, value_delimiter = ',', value_name = "RULES")]
    ignore: Vec<String>,
}

#[derive(Args)]
struct FormatArgs {
    /// Files or directories to format; `-` formats standard input to standard output
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,

    /// Change nothing; exit with status 1 if any file would be reformatted
    #[arg(long)]
    check: bool,

    /// Formatting style, overriding the configuration
    #[arg(long, value_enum)]
    style: Option<StyleName>,
}

#[derive(Args)]
struct RulesArgs {
    #[arg(long, value_enum, default_value = "text", value_name = "FORMAT")]
    output_format: OutputFormat,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // Run on a thread with a large stack; see `STACK_SIZE`.
    let worker = std::thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(move || run(cli));
    let result = match worker.map(|handle| handle.join()) {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(anyhow::anyhow!(
            "weavejig crashed; this is a bug, please report it"
        )),
        Err(error) => Err(error.into()),
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(EXIT_ERROR)
        }
    }
}

fn run(cli: Cli) -> Result<u8> {
    rayon::ThreadPoolBuilder::new()
        .stack_size(STACK_SIZE)
        .build_global()
        .ok();
    let cwd = std::env::current_dir().context("cannot determine the current directory")?;
    let loaded = config::load(cli.config.as_deref(), &cwd)?;
    match cli.command {
        Command::Check(args) => check(args, &loaded),
        Command::Format(args) => format_command(args, &loaded),
        Command::Rules(args) => rules(args),
    }
}

fn is_stdin(paths: &[PathBuf]) -> bool {
    matches!(paths, [path] if path == Path::new("-"))
}

fn read_stdin() -> Result<String> {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .context("cannot read standard input")?;
    Ok(text)
}

fn read_source(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))
}

// --- check ------------------------------------------------------------------

struct Checked {
    diagnostics: Vec<output::JsonDiagnostic>,
    fixed: usize,
}

/// Lints one source. `path` is `None` for standard input.
fn check_source(
    file: &str,
    path: Option<&Path>,
    text: &str,
    rules: &RuleSet,
    fix: bool,
) -> (Checked, Option<String>) {
    let source = Source::new(text, path);
    if fix {
        let outcome = fix_all(source, rules);
        let diagnostics = output::convert(file, &outcome.text, &outcome.remaining);
        let changed = (outcome.text != text).then_some(outcome.text);
        (
            Checked {
                diagnostics,
                fixed: outcome.fixed,
            },
            changed,
        )
    } else {
        let diagnostics = output::convert(file, text, &lint(source, rules));
        (
            Checked {
                diagnostics,
                fixed: 0,
            },
            None,
        )
    }
}

fn check(args: CheckArgs, loaded: &config::Loaded) -> Result<u8> {
    let rules = loaded.config.rules(&config::SelectionOverrides {
        select: &args.select,
        extend_select: &args.extend_select,
        ignore: &args.ignore,
    })?;
    let per_file_ignores = loaded.config.per_file_ignores(&loaded.root)?;

    // Files that could not be read or written. Each is reported and the
    // rest are still checked, so one bad file does not hide the others.
    let mut failed = 0;
    let results: Vec<Checked> = if is_stdin(&args.paths) {
        let text = read_stdin()?;
        let (checked, fixed_text) = check_source("<stdin>", None, &text, &rules, args.fix);
        if args.fix {
            // With `--fix`, standard output carries the fixed source, so the
            // report goes to standard error.
            print!("{}", fixed_text.as_deref().unwrap_or(&text));
            let report = report(vec![checked]);
            eprint!("{}", render(&report, args.output_format)?);
            return Ok(if report.summary.diagnostics > 0 {
                EXIT_FINDINGS
            } else {
                EXIT_OK
            });
        }
        vec![checked]
    } else {
        let files = files::discover(&args.paths, &loaded.root, &loaded.config.exclude)?;
        let outcomes: Vec<Result<Checked>> = files
            .par_iter()
            .map(|path| {
                let text = read_source(path)?;
                let file = path.display().to_string();
                let ignored = per_file_ignores.for_path(path);
                let narrowed;
                let rules = if ignored.is_empty() {
                    &rules
                } else {
                    narrowed = rules.without(&ignored);
                    &narrowed
                };
                let (checked, fixed_text) = check_source(&file, Some(path), &text, rules, args.fix);
                if let Some(fixed_text) = fixed_text {
                    fs::write(path, fixed_text)
                        .with_context(|| format!("cannot write {}", path.display()))?;
                }
                Ok(checked)
            })
            .collect();
        let mut results = Vec::with_capacity(outcomes.len());
        for outcome in outcomes {
            match outcome {
                Ok(checked) => results.push(checked),
                Err(error) => {
                    failed += 1;
                    eprintln!("error: {error:#}");
                }
            }
        }
        results
    };

    let report = report(results);
    print!("{}", render(&report, args.output_format)?);
    if failed > 0 {
        eprintln!("{} could not be checked.", count(failed, "file"));
        return Ok(EXIT_ERROR);
    }
    Ok(if report.summary.diagnostics > 0 {
        EXIT_FINDINGS
    } else {
        EXIT_OK
    })
}

fn report(results: Vec<Checked>) -> Report {
    let mut summary = Summary {
        files_checked: results.len(),
        ..Summary::default()
    };
    let mut diagnostics = Vec::new();
    for checked in results {
        summary.fixed += checked.fixed;
        summary.files_with_diagnostics += usize::from(!checked.diagnostics.is_empty());
        diagnostics.extend(checked.diagnostics);
    }
    summary.diagnostics = diagnostics.len();
    summary.fixable = diagnostics.iter().filter(|d| d.fix.is_some()).count();
    Report {
        version: output::FORMAT_VERSION,
        diagnostics,
        summary,
    }
}

fn render(report: &Report, format: OutputFormat) -> Result<String> {
    Ok(match format {
        OutputFormat::Text => output::render_text(report),
        OutputFormat::Json => serde_json::to_string_pretty(report)? + "\n",
    })
}

// --- format -----------------------------------------------------------------

enum Formatted {
    Unchanged,
    Changed,
    Failed(String),
}

fn format_file(path: &Path, options: &FormatOptions, write: bool) -> Result<Formatted> {
    let text = read_source(path)?;
    match format(&text, options) {
        Ok(formatted) if formatted == text => Ok(Formatted::Unchanged),
        Ok(formatted) => {
            if write {
                fs::write(path, formatted)
                    .with_context(|| format!("cannot write {}", path.display()))?;
            }
            Ok(Formatted::Changed)
        }
        Err(error) => Ok(Formatted::Failed(describe(path, &text, &error))),
    }
}

fn describe(path: &Path, text: &str, error: &FormatError) -> String {
    match error {
        FormatError::Syntax(errors) => {
            let index = weavejig_syntax::LineIndex::new(text);
            let mut lines = Vec::new();
            for error in errors {
                let at = index.line_col(text, error.range.start());
                lines.push(format!(
                    "{}:{}:{}: syntax-error: {}",
                    path.display(),
                    at.line,
                    at.column,
                    error.message
                ));
            }
            lines.join("\n")
        }
        FormatError::Internal(_) => format!("{}: {error}", path.display()),
    }
}

fn format_command(args: FormatArgs, loaded: &config::Loaded) -> Result<u8> {
    let options = loaded.config.format_options(args.style);

    if is_stdin(&args.paths) {
        let text = read_stdin()?;
        return match format(&text, &options) {
            Ok(formatted) => {
                if !args.check {
                    std::io::stdout().write_all(formatted.as_bytes())?;
                }
                Ok(if args.check && formatted != text {
                    EXIT_FINDINGS
                } else {
                    EXIT_OK
                })
            }
            Err(error) => {
                eprintln!("{}", describe(Path::new("<stdin>"), &text, &error));
                Ok(EXIT_ERROR)
            }
        };
    }

    let files = files::discover(&args.paths, &loaded.root, &loaded.config.exclude)?;
    // A file that cannot be read or written fails on its own; the others
    // are still formatted and reported.
    let results: Vec<(&PathBuf, Formatted)> = files
        .par_iter()
        .map(|path| {
            let result = format_file(path, &options, !args.check)
                .unwrap_or_else(|error| Formatted::Failed(format!("error: {error:#}")));
            (path, result)
        })
        .collect();

    let (mut changed, mut unchanged, mut failed) = (0, 0, 0);
    for (path, result) in &results {
        match result {
            Formatted::Unchanged => unchanged += 1,
            Formatted::Changed => {
                changed += 1;
                if args.check {
                    println!("Would reformat: {}", path.display());
                }
            }
            Formatted::Failed(message) => {
                failed += 1;
                eprintln!("{message}");
            }
        }
    }
    let verb = if args.check {
        "would be reformatted"
    } else {
        "reformatted"
    };
    let mut summary = format!("{} {verb}, {} unchanged", count(changed, "file"), unchanged);
    if failed > 0 {
        summary.push_str(&format!(", {failed} could not be formatted"));
    }
    println!("{summary}.");

    Ok(if failed > 0 {
        EXIT_ERROR
    } else if args.check && changed > 0 {
        EXIT_FINDINGS
    } else {
        EXIT_OK
    })
}

// --- rules ------------------------------------------------------------------

fn rules(args: RulesArgs) -> Result<u8> {
    match args.output_format {
        OutputFormat::Json => {
            let rules: Vec<_> = all_rules()
                .iter()
                .map(|rule| {
                    let meta = rule.meta();
                    let options: Vec<_> = meta
                        .options
                        .iter()
                        .map(|option| {
                            let default = match option.default {
                                OptionDefault::Bool(value) => serde_json::json!(value),
                                OptionDefault::Int(value) => serde_json::json!(value),
                                OptionDefault::Str(value) | OptionDefault::Regex(value) => {
                                    serde_json::json!(value)
                                }
                                OptionDefault::List(values) => serde_json::json!(values),
                            };
                            serde_json::json!({
                                "name": option.name,
                                "type": option.default.type_name(),
                                "default": default,
                                "help": option.help,
                            })
                        })
                        .collect();
                    serde_json::json!({
                        "id": meta.id,
                        "summary": meta.summary,
                        "fixable": meta.fixable,
                        "default": meta.default,
                        "checkstyle": meta.checkstyle,
                        "pmd": meta.pmd.map(|rule| rule.name),
                        "options": options,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&rules)?);
        }
        OutputFormat::Text => {
            let width = all_rules()
                .iter()
                .map(|rule| rule.meta().id.len())
                .max()
                .unwrap_or(0);
            for rule in all_rules() {
                let meta = rule.meta();
                let fixable = if meta.fixable { "fix" } else { "   " };
                // Rules that run only when selected are marked.
                let selected = if meta.default { "      " } else { "opt-in" };
                println!(
                    "{:width$}  {fixable}  {selected}  {}",
                    meta.id, meta.summary
                );
            }
        }
    }
    Ok(EXIT_OK)
}
