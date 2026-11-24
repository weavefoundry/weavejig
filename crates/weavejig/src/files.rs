//! Finding the Java files to work on.

use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;

/// The absolute form of `path`, with `.` and `..` resolved by name alone
/// (without following symbolic links).
pub(crate) fn absolute(path: &Path) -> std::io::Result<PathBuf> {
    let mut out = PathBuf::new();
    for component in std::path::absolute(path)?.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    Ok(out)
}

/// Expands `paths` into Java files. Directories are walked, honouring
/// `.gitignore` and the configured `exclude` globs; files named explicitly
/// are always included, whatever their extension.
pub fn discover(paths: &[PathBuf], config_root: &Path, exclude: &[String]) -> Result<Vec<PathBuf>> {
    // Patterns are relative to the configuration file, not to the current
    // directory, so they are matched against absolute paths.
    let config_root = absolute(config_root)?;
    let mut overrides = OverrideBuilder::new(&config_root);
    for glob in exclude {
        // In an override, a leading `!` means "ignore".
        overrides
            .add(&format!("!{glob}"))
            .with_context(|| format!("invalid `exclude` pattern `{glob}`"))?;
    }
    let overrides = overrides.build()?;

    let mut files = Vec::new();
    for path in paths {
        if path.is_file() {
            files.push(path.clone());
            continue;
        }
        if !path.exists() {
            anyhow::bail!("no such file or directory: {}", path.display());
        }
        let overrides = overrides.clone();
        let walk = WalkBuilder::new(path)
            .filter_entry(move |entry| {
                let is_dir = entry.file_type().is_some_and(|kind| kind.is_dir());
                absolute(entry.path())
                    .map_or(true, |path| !overrides.matched(path, is_dir).is_ignore())
            })
            .build();
        for entry in walk {
            let entry = entry?;
            let is_java = entry.path().extension().is_some_and(|ext| ext == "java");
            if is_java && entry.file_type().is_some_and(|kind| kind.is_file()) {
                // `./Foo.java` reads better as `Foo.java`.
                let path = entry.path().strip_prefix("./").unwrap_or(entry.path());
                files.push(path.to_path_buf());
            }
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}
