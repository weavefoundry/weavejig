//! `cargo xtask fetch`: downloads the reference tools and corpus.

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use crate::manifest::{Codebase, Manifest, Reference};

fn run(command: &mut Command) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("cannot run {command:?}"))?;
    if !status.success() {
        bail!("{command:?} failed with {status}");
    }
    Ok(())
}

fn sha256(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    Ok(Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub fn references(manifest: &Manifest) -> Result<()> {
    for (name, reference) in &manifest.references {
        fetch_reference(name, reference)?;
    }
    Ok(())
}

fn fetch_reference(name: &str, reference: &Reference) -> Result<()> {
    let jar = reference.jar_path();
    if jar.is_file() && sha256(&jar)? == reference.sha256 {
        println!("{name} {}: already downloaded", reference.version);
        return Ok(());
    }
    println!(
        "{name} {}: downloading {}",
        reference.version, reference.url
    );
    let dir = jar.parent().context("jar path has a parent")?;
    fs::create_dir_all(dir)?;
    let partial = jar.with_extension("partial");
    run(Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error"])
        .arg("--output")
        .arg(&partial)
        .arg(&reference.url))?;
    // Never leave an unverified jar where the harness would run it.
    let actual = sha256(&partial)?;
    if actual != reference.sha256 {
        fs::remove_file(&partial).ok();
        bail!(
            "{name}: checksum mismatch for {}\n  expected {}\n  actual   {actual}",
            reference.url,
            reference.sha256
        );
    }
    fs::rename(&partial, &jar)?;
    Ok(())
}

pub fn corpus(manifest: &Manifest, set: &str) -> Result<()> {
    let codebases = manifest.codebases(set);
    if codebases.is_empty() {
        bail!("scoring.toml has no corpus entries in set `{set}`");
    }
    for codebase in codebases {
        fetch_codebase(codebase)?;
    }
    Ok(())
}

/// Records which commit a checkout holds, so an up-to-date one is skipped.
const REV_FILE: &str = ".weavejig-rev";

pub fn is_fetched(codebase: &Codebase) -> bool {
    fs::read_to_string(codebase.checkout_dir().join(REV_FILE))
        .is_ok_and(|rev| rev.trim() == codebase.rev)
}

fn fetch_codebase(codebase: &Codebase) -> Result<()> {
    if is_fetched(codebase) {
        println!("{}: already at {}", codebase.name, &codebase.rev[..12]);
        return Ok(());
    }
    let dir = codebase.checkout_dir();
    println!(
        "{}: fetching {} at {}",
        codebase.name,
        codebase.url,
        &codebase.rev[..12]
    );
    fs::create_dir_all(&dir)?;
    let git = |args: &[&str]| run(Command::new("git").arg("-C").arg(&dir).args(args));
    if !dir.join(".git").exists() {
        git(&["init", "--quiet"])?;
        git(&["remote", "add", "origin", &codebase.url])?;
    }
    // Only the one pinned commit, without history.
    git(&["fetch", "--quiet", "--depth", "1", "origin", &codebase.rev])?;
    git(&[
        "-c",
        "advice.detachedHead=false",
        "checkout",
        "--quiet",
        "--force",
        "FETCH_HEAD",
    ])?;
    fs::write(dir.join(REV_FILE), &codebase.rev)?;
    Ok(())
}
