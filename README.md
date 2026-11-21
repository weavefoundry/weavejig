# WeaveJig

A fast, native linter and formatter for Java in a single binary. Written in Rust. No JVM required.

## Goal

Replace google-java-format, Checkstyle, and PMD with one tool and one config file.

## Outcomes

1. **Formatter:** output identical to google-java-format, verified against large open-source codebases.
2. **Linter:** the most-used syntax-level Checkstyle and PMD rules, with automatic fixes.
3. **One binary, one config:** starts in milliseconds and works with Maven, Gradle, or no build tool.
4. **Machine-readable output:** JSON diagnostics with exact locations and fixes, for editors, CI, and coding agents.

## Non-goals

- Type-aware analysis (Error Prone, NullAway)
- Kotlin and other JVM languages
- Building or dependency management

## Status

Early development. Not yet usable.
