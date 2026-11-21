//! Finding a JDK to run the reference tools with.

use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Jdk {
    pub java: PathBuf,
    /// The feature release: 21 for `21.0.4`.
    pub feature: u32,
    pub version: String,
}

/// Parses the version out of `java -version` output, such as
/// `openjdk version "21.0.4" 2024-07-16`.
fn parse_version(output: &str) -> Option<(u32, String)> {
    let version = output.split('"').nth(1)?;
    let mut parts = version.split(['.', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    // Java 8 and earlier call themselves 1.8, 1.7, ...
    let feature = if first == 1 {
        parts.next()?.parse().ok()?
    } else {
        first
    };
    Some((feature, version.to_string()))
}

fn probe(java: PathBuf) -> Option<Jdk> {
    let output = Command::new(&java).arg("-version").output().ok()?;
    // `java -version` writes to standard error.
    let text = String::from_utf8_lossy(&output.stderr);
    let (feature, version) = parse_version(&text)?;
    Some(Jdk {
        java,
        feature,
        version,
    })
}

/// Every JDK that can be found, in order of preference: `WEAVEJIG_JAVA_HOME`,
/// `JAVA_HOME`, `java` on the `PATH`, and on macOS whatever
/// `/usr/libexec/java_home` knows about.
fn candidates() -> Vec<Jdk> {
    let mut paths = Vec::new();
    for variable in ["WEAVEJIG_JAVA_HOME", "JAVA_HOME"] {
        if let Some(home) = std::env::var_os(variable).filter(|home| !home.is_empty()) {
            paths.push(PathBuf::from(home).join("bin").join("java"));
        }
    }
    paths.push(PathBuf::from("java"));
    if cfg!(target_os = "macos") {
        if let Ok(output) = Command::new("/usr/libexec/java_home").arg("-V").output() {
            // Lines look like: `21.0.4 (arm64) "Vendor" - "Name" /path/to/Home`
            let listing = String::from_utf8_lossy(&output.stderr).into_owned();
            for line in listing.lines() {
                if let Some(home) = line.rsplit(' ').next().filter(|home| home.starts_with('/')) {
                    paths.push(PathBuf::from(home).join("bin").join("java"));
                }
            }
        }
    }
    paths.into_iter().filter_map(probe).collect()
}

/// The preferred JDK that is at least `min_feature`, or an explanation of
/// what was found instead.
pub fn locate(min_feature: u32) -> Result<Jdk, String> {
    let found = candidates();
    if let Some(jdk) = found.iter().find(|jdk| jdk.feature >= min_feature) {
        return Ok(jdk.clone());
    }
    Err(match found.first() {
        Some(jdk) => format!(
            "needs JDK {min_feature} or newer, found {}; set WEAVEJIG_JAVA_HOME",
            jdk.version
        ),
        None => format!("needs JDK {min_feature} or newer, found none; set WEAVEJIG_JAVA_HOME"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        let parse = |text| parse_version(text).map(|(feature, _)| feature);
        assert_eq!(
            parse("openjdk version \"21.0.4\" 2024-07-16\nOpenJDK Runtime"),
            Some(21)
        );
        assert_eq!(parse("java version \"1.8.0_402\""), Some(8));
        assert_eq!(parse("openjdk version \"25\" 2025-09-16"), Some(25));
        assert_eq!(parse("openjdk version \"26-ea\" 2026-03-17"), Some(26));
        assert_eq!(parse("command not found"), None);
    }
}
