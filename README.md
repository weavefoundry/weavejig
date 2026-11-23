# WeaveJig

A fast, native linter and formatter for Java in a single binary. Written in Rust. No JVM required.

## Goal

Replace google-java-format, Checkstyle, and PMD with one tool and one config file.

## Outcomes

1. **Formatter:** output identical to google-java-format, verified against large open-source codebases.
2. **Linter:** the most-used syntax-level Checkstyle and PMD rules, with automatic fixes.
3. **One binary, one config:** starts in milliseconds and works with Maven, Gradle, or no build tool.
4. **Machine-readable output:** JSON diagnostics with exact locations and fixes, for editors, CI, and coding agents.

## Measuring progress

Each part is scored against an existing tool, over a corpus of open-source Java:

| Part | Reference | Score |
|---|---|---|
| Parser | `javac` | Share of files accepted or rejected the same way |
| Formatter | google-java-format | Share of files with identical output |
| Linter | Checkstyle, PMD | Share of diagnostics matching by rule and line |

- Build the scoring harness first: one command that prints these scores.
- Keep a held-out set of codebases that is never used during development.
- Reimplement from documentation and observed behaviour. Do not translate the reference tools' source (Checkstyle is LGPL).

## Non-goals

- Type-aware analysis (Error Prone, NullAway)
- Kotlin and other JVM languages
- Building or dependency management

## Status

Early development. Everything below runs, but nothing is at parity yet: do not rely on it for real code.

| Part | What exists |
|---|---|
| Parser | Hand-written, lossless, error-tolerant, written for the Java 21 grammar. Agrees with `javac` on 6,636 of 6,640 dev-corpus files and on 2,548 of 2,549 held-out files. The 4 dev disagreements are JUnit tests that use unnamed variables (`_`), which are Java 22: `javac` 21 rejects them. |
| Formatter | Formats whole files in google-java-format style, Javadoc included. 92.8% of dev-corpus files (6,157 of 6,636) come out identical to google-java-format 2.0.0, and 80.9% of held-out files (2,063 of 2,549). Most of the rules were tuned on guava, gson and commons-lang, where 97.5% match; jsoup, zxing and JUnit were added to the dev corpus later and match less well. As google-java-format does, long string literals are split at word boundaries, modifiers are put in JLS order, and unused imports are removed. Not done: a tail of rarer wrapping and comment-placement cases differs. |
| Linter | Six Checkstyle rules, five with fixes. Their diagnostics match Checkstyle on 1,523 of 1,527 in the dev corpus and all 1,083 in the held-out corpus. No PMD rules. |
| CLI | `check`, `format`, `rules`; text and JSON output; `weavejig.toml`. |
| Scoring | `cargo xtask score` for the parser (`javac`), formatter (google-java-format) and linter (Checkstyle). PMD is not wired up. |

The formatter never changes anything but whitespace, import order, the wrapping of comments, the splitting of long string literals, the order of modifier keywords and which imports there are: it checks its own output against the input, and refuses to write a file if any token would differ or any comment would gain or lose text. A string literal may only become, or be merged from, literals joined by `+` whose text, put together, is the same. Modifier keywords may only trade places with keywords not separated from them by an annotation. An import may only go if nothing in the output uses its simple name, or if it imports from `java.lang` or the file's own package; wildcard imports always stay.

## Usage

```bash
cargo install --path crates/weavejig
```

```bash
weavejig check                      # lint the current directory
weavejig check --fix src/           # apply fixes
weavejig check --output-format json # diagnostics for tools
weavejig format                     # format in place
weavejig format --check             # exit 1 if anything would change
weavejig format - < Foo.java        # format standard input
weavejig rules                      # list the lint rules
```

Directories are searched for `.java` files, skipping anything in `.gitignore`.

Exit status: `0` nothing to report, `1` problems found or files need formatting, `2` WeaveJig could not do what was asked.

### Configuration

WeaveJig reads the nearest `weavejig.toml` in the current directory or above it. Every key is optional.

```toml
# Paths never checked or formatted, relative to this file.
exclude = ["**/generated/**"]

[format]
style = "google" # or "aosp"

[lint]
select = ["upper-ell", "need-braces"] # default: every rule
ignore = ["avoid-star-import"]
```

### JSON output

`weavejig check --output-format json` prints one object:

```json
{
  "version": 1,
  "diagnostics": [
    {
      "file": "src/Foo.java",
      "rule": "upper-ell",
      "severity": "warning",
      "message": "long literal should use an upper-case `L` suffix",
      "range": {
        "start": { "line": 6, "column": 33, "offset": 102 },
        "end": { "line": 6, "column": 36, "offset": 105 }
      },
      "fix": {
        "description": "use an upper-case `L`",
        "edits": [
          {
            "range": {
              "start": { "line": 6, "column": 35, "offset": 104 },
              "end": { "line": 6, "column": 36, "offset": 105 }
            },
            "replacement": "L"
          }
        ]
      }
    }
  ],
  "summary": { "files_checked": 1, "files_with_diagnostics": 1, "diagnostics": 1, "fixable": 1, "fixed": 0 }
}
```

Lines and columns are 1-based; columns count Unicode code points. `offset` is a 0-based byte offset into the UTF-8 file. Ranges are half-open. `fix` is `null` when a diagnostic has no automatic fix; a fix's edits never overlap and must be applied together. Syntax errors are reported as rule `syntax-error` with severity `error`.

### Rules

| Rule | Mirrors Checkstyle | Fix |
|---|---|---|
| `array-type-style` | ArrayTypeStyle | yes |
| `avoid-star-import` | AvoidStarImport | no |
| `empty-statement` | EmptyStatement | yes |
| `modifier-order` | ModifierOrder | yes |
| `need-braces` | NeedBraces | yes |
| `upper-ell` | UpperEll | yes |

## Development

Requires Rust 1.88 or newer. Scoring also needs JDK 21 or newer.

```bash
cargo test --workspace
cargo xtask score
```

### Layout

| Path | What |
|---|---|
| `crates/weavejig-syntax` | Lexer, parser and lossless syntax tree |
| `crates/weavejig-formatter` | Syntax tree → layout document → text |
| `crates/weavejig-linter` | Lint rules and fixes, one module per rule |
| `crates/weavejig` | The command-line binary |
| `xtask` | The scoring harness |
| `scoring.toml` | Pinned reference tools and corpus |
| `reference/` | The `javac` parse helper the harness runs |
| `corpus/smoke/` | A small in-repository corpus |

### Scoring

`cargo xtask score` prints the scores from [Measuring progress](#measuring-progress). With no arguments it uses the smoke corpus in this repository, and measures whatever it has the references for; the parser score needs only a JDK.

```bash
cargo xtask fetch references          # google-java-format and Checkstyle jars
cargo xtask fetch corpus --set dev    # the development codebases
cargo xtask score --set dev --show 20 # scores, and the first 20 disagreements per part
```

The formatter is scored twice. The second row, `scrambled`, formats each file after its whitespace has been rewritten into another style (other indentation, other line wrapping, braces on their own line), keeping every token and comment. Much of the corpus is already in google-java-format style, so the first row alone would mostly test leaving formatted code alone. To see the scrambled input for a disagreement:

```bash
cargo xtask scramble path/to/File.java
```

Reference tool versions and corpus commits are pinned in [`scoring.toml`](scoring.toml); downloads go to `.weavejig/` and are checked against the pinned SHA-256. The harness looks for a JDK in `WEAVEJIG_JAVA_HOME`, `JAVA_HOME`, then `PATH`.

The `held-out` set is for measuring only. `cargo xtask score --set held-out` prints totals and never lists individual disagreements: do not inspect its files to fix a score.

### Adding a lint rule

1. Add a module in `crates/weavejig-linter/src/rules/` with a unit struct implementing `Rule` and a `static META` naming the Checkstyle check it mirrors.
2. List it in `ALL` in `rules/mod.rs`.
3. Add cases to `crates/weavejig-linter/tests/rules.rs`.
4. Run `cargo xtask score --set dev` and compare with Checkstyle.

### Changing the formatter

Formatting rules live in `crates/weavejig-formatter/src/format.rs`; a node kind without a rule is laid out on one line by the spacing table in `spacing.rs`. Long string literals are split after layout, in `strings.rs`. Fixtures in `crates/weavejig-formatter/tests/fixtures/` record current output so that changes show up in review; see the README there.
