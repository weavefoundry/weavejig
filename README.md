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

The linter has a second score, coverage: of the diagnostics that Checkstyle's own `sun_checks` and `google_checks` configurations report, the share that comes from checks WeaveJig has a rule for. Agreement says how good the rules are; coverage says how many there are.

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
| Formatter | Formats whole files in google-java-format style, Javadoc included. 94.1% of dev-corpus files (6,247 of 6,636) come out identical to google-java-format 2.0.0, and 81.4% of held-out files (2,075 of 2,549). Most of the rules were tuned on guava, gson and commons-lang, where 97.7% match; jsoup, zxing and JUnit were added to the dev corpus later and match less well. As google-java-format does, long string literals are split at word boundaries, long trailing `//` comments continue on lines under their start, modifiers are put in JLS order, and unused imports are removed. Not done: a tail of rarer wrapping and comment-placement cases differs. |
| Linter | 71 rules, 11 with fixes; 46 run by default and the rest are opt-in. 69 mirror Checkstyle checks: with default options their diagnostics match Checkstyle on 177,933 of 177,946 in the dev corpus and on 55,979 of 55,999 in the held-out corpus. 10 mirror PMD rules, matching on 13,071 of 13,091 and 4,618 of 4,650. Rules take options, and diagnostics can be suppressed in the source. Coverage: setting aside layout checks, which the formatter replaces, the rules account for 84% of what Checkstyle's `sun_checks` reports on the dev corpus and 79% of what `google_checks` reports (83% and 36% on the held-out corpus); nearly all of the rest is Checkstyle's Javadoc checks, which have no rules yet. Not done: Javadoc rules, most of PMD, and anything that needs types. |
| CLI | `check`, `format`, `rules`; text and JSON output; `weavejig.toml`, with rule selection, rule options and per-file ignores. |
| Scoring | `cargo xtask score` for the parser (`javac`), formatter (google-java-format) and linter (Checkstyle and PMD), rule by rule, with coverage against Checkstyle's bundled configurations. |

The formatter never changes anything but whitespace, import order, the wrapping of comments, the splitting of long string literals, the order of modifier keywords and which imports there are: it checks its own output against the input, and refuses to write a file if any token would differ or any comment would gain or lose text. A string literal may only become, or be merged from, literals joined by `+` whose text, put together, is the same. Modifier keywords may only trade places with keywords not separated from them by an annotation. An import may only go if nothing in the output uses its simple name, or if it imports from `java.lang` or the file's own package; wildcard imports always stay.

## Usage

```bash
cargo install --path crates/weavejig
```

```bash
weavejig check                      # lint the current directory
weavejig check --fix src/           # apply fixes
weavejig check --output-format json # diagnostics for tools
weavejig check --select all         # every rule, not only the default ones
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
select = ["upper-ell", "need-braces"] # default: the rules that are on by default
extend-select = ["magic-number"]      # these as well
ignore = ["avoid-star-import"]        # but never these

# Rules to skip where a glob matches, relative to this file.
[lint.per-file-ignores]
"**/src/test/**" = ["magic-number", "method-name"]

# Options of a rule.
[lint.rules.magic-number]
ignore-numbers = [-1, 0, 1, 2, 100]

[lint.rules.line-length]
max = 120
```

`select = ["all"]` runs every rule. `weavejig rules` lists the rules, marking the opt-in ones; `weavejig rules --output-format json` adds each rule's options with their types and defaults. Options that are patterns are [regular expressions as Rust's `regex` crate reads them](https://docs.rs/regex/latest/regex/#syntax), which is the common syntax without look-around and backreferences.

### Suppressing diagnostics

```java
long a = 1l; // weavejig-ignore: upper-ell

// weavejig-ignore: upper-ell, magic-number
long b = 250l;

@SuppressWarnings("weavejig:magic-number")
int port() {
  return 8080;
}
```

- A comment that starts with `weavejig-ignore` covers its line if it comes after code; on a line of its own, the next line. Without rule ids it covers every rule.
- `// weavejig-ignore-file: rule-id` covers the whole file.
- `@SuppressWarnings("weavejig:rule-id")` covers the declaration it is on; `"weavejig"` and `"all"` cover every rule.

Code that already suppresses Checkstyle or PMD needs no changes. For the rules that mirror their checks, WeaveJig honours `@SuppressWarnings("checkstyle:magicnumber")` and `// CHECKSTYLE:OFF` up to `// CHECKSTYLE:ON`, and `@SuppressWarnings("PMD.RuleName")`, `@SuppressWarnings("PMD")` and `// NOPMD`.

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

A rule mirrors a Checkstyle check unless marked as PMD's, and with default options reports what that check reports with its own. Three rules differ on purpose. `line-length` allows 100 columns, the formatter's width, where Checkstyle allows 80. `final-class` and `redundant-modifier` leave out a few cases where Checkstyle asks for a change that would not compile or would change what the code means.

| Rule | Mirrors | Default | Fix | Options |
|---|---|---|---|---|
| `abbreviation-as-word-in-name` | AbbreviationAsWordInName |  |  | `allowed-abbreviation-length`, `allowed-abbreviations`, `ignore-final`, `ignore-static`, `ignore-static-final`, `ignore-overridden-methods` |
| `array-type-style` | ArrayTypeStyle | yes | yes |  |
| `avoid-nested-blocks` | AvoidNestedBlocks | yes |  | `allow-in-switch-case` |
| `avoid-star-import` | AvoidStarImport | yes |  |  |
| `avoid-static-import` | AvoidStaticImport |  |  | `excludes` |
| `catch-parameter-name` | CatchParameterName |  |  | `format` |
| `class-type-parameter-name` | ClassTypeParameterName |  |  | `format` |
| `constant-name` | ConstantName |  |  | `format` |
| `covariant-equals` | CovariantEquals | yes |  |  |
| `declaration-order` | DeclarationOrder |  |  | `ignore-constructors`, `ignore-modifiers` |
| `default-comes-last` | DefaultComesLast, PMD DefaultLabelNotLastInSwitch | yes |  |  |
| `design-for-extension` | DesignForExtension |  |  | `ignored-annotations`, `required-javadoc-phrase` |
| `empty-block` | EmptyBlock, PMD EmptyControlStatement | yes |  |  |
| `empty-catch-block` | EmptyCatchBlock | yes |  | `exception-variable-name`, `comment-format` |
| `empty-statement` | EmptyStatement | yes | yes |  |
| `equals-hash-code` | EqualsHashCode, PMD OverrideBothEqualsAndHashcode | yes |  |  |
| `explicit-initialization` | ExplicitInitialization, PMD RedundantFieldInitializer | yes | yes | `only-object-references` |
| `fall-through` | FallThrough, PMD ImplicitSwitchFallThrough | yes |  | `relief-pattern`, `check-last-case-group` |
| `file-length` | FileLength |  |  | `max` |
| `file-tab-character` | FileTabCharacter |  |  | `each-line` |
| `final-class` | FinalClass | yes |  |  |
| `final-parameters` | FinalParameters |  | yes | `ignore-primitive-types` |
| `hidden-field` | HiddenField |  |  | `ignore-constructor-parameter`, `ignore-setter`, `setter-can-return-its-class`, `ignore-abstract-methods`, `ignore-format` |
| `hide-utility-class-constructor` | HideUtilityClassConstructor | yes |  |  |
| `illegal-import` | IllegalImport | yes |  | `packages`, `classes` |
| `inner-assignment` | InnerAssignment | yes |  |  |
| `interface-is-type` | InterfaceIsType |  |  | `allow-marker-interfaces` |
| `interface-type-parameter-name` | InterfaceTypeParameterName |  |  | `format` |
| `lambda-parameter-name` | LambdaParameterName | yes |  | `format` |
| `line-length` | LineLength |  |  | `max`, `ignore-pattern`, `tab-width` |
| `local-final-variable-name` | LocalFinalVariableName | yes |  | `format` |
| `local-variable-name` | LocalVariableName | yes |  | `format` |
| `magic-number` | MagicNumber |  |  | `ignore-numbers`, `ignore-hash-code-method`, `ignore-annotation`, `ignore-field-declaration`, `ignore-annotation-element-defaults` |
| `member-name` | MemberName | yes |  | `format` |
| `method-length` | MethodLength |  |  | `max`, `count-empty` |
| `method-name` | MethodName, PMD MethodNamingConventions |  |  | `format`, `allow-class-name` |
| `method-type-parameter-name` | MethodTypeParameterName |  |  | `format` |
| `missing-deprecated` | MissingDeprecated | yes |  |  |
| `missing-override` | MissingOverride | yes |  |  |
| `missing-switch-default` | MissingSwitchDefault | yes |  |  |
| `modifier-order` | ModifierOrder | yes | yes |  |
| `multiple-variable-declarations` | MultipleVariableDeclarations, PMD OneDeclarationPerLine | yes |  |  |
| `need-braces` | NeedBraces, PMD ControlStatementBraces | yes | yes |  |
| `newline-at-end-of-file` | NewlineAtEndOfFile | yes | yes |  |
| `no-clone` | NoClone |  |  |  |
| `no-finalizer` | NoFinalizer | yes |  |  |
| `one-statement-per-line` | OneStatementPerLine | yes |  |  |
| `one-top-level-class` | OneTopLevelClass | yes |  |  |
| `outer-type-filename` | OuterTypeFilename | yes |  |  |
| `package-name` | PackageName | yes |  | `format` |
| `parameter-name` | ParameterName | yes |  | `format` |
| `parameter-number` | ParameterNumber |  |  | `max`, `ignore-overridden-methods` |
| `pattern-variable-name` | PatternVariableName | yes |  | `format` |
| `record-component-name` | RecordComponentName | yes |  | `format` |
| `record-type-parameter-name` | RecordTypeParameterName |  |  | `format` |
| `redundant-import` | RedundantImport | yes | yes |  |
| `redundant-modifier` | RedundantModifier | yes | yes |  |
| `regexp-singleline` | RegexpSingleline | yes |  | `format`, `message` |
| `simplify-boolean-expression` | SimplifyBooleanExpression | yes |  |  |
| `simplify-boolean-return` | SimplifyBooleanReturn | yes |  |  |
| `static-variable-name` | StaticVariableName | yes |  | `format` |
| `string-literal-equality` | StringLiteralEquality | yes |  |  |
| `super-clone` | SuperClone | yes |  |  |
| `super-finalize` | SuperFinalize | yes |  |  |
| `todo-comment` | TodoComment |  |  | `format` |
| `type-name` | TypeName | yes |  | `format` |
| `uncommented-empty-constructor` | PMD UncommentedEmptyConstructor |  |  |  |
| `uncommented-empty-method-body` | PMD UncommentedEmptyMethodBody |  |  |  |
| `unused-imports` | UnusedImports | yes | yes | `process-javadoc` |
| `upper-ell` | UpperEll | yes | yes |  |
| `visibility-modifier` | VisibilityModifier |  |  | `package-allowed`, `protected-allowed`, `public-member-pattern`, `allow-public-final-fields`, `ignore-annotations` |

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
| `scoring.toml` | Pinned reference tools (google-java-format, Checkstyle, PMD) and corpus |
| `reference/` | The `javac` parse helper the harness runs |
| `corpus/smoke/` | A small in-repository corpus |
| `corpus/smoke/lint/` | Hand-written probes that trip every lint rule |

### Scoring

`cargo xtask score` prints the scores from [Measuring progress](#measuring-progress). With no arguments it uses the smoke corpus in this repository, and measures whatever it has the references for; the parser score needs only a JDK.

```bash
cargo xtask fetch references          # google-java-format, Checkstyle and PMD
cargo xtask fetch corpus --set dev    # the development codebases
cargo xtask score --set dev --show 20 # scores, and the first 20 disagreements per part
```

The formatter is scored twice. The second row, `scrambled`, formats each file after its whitespace has been rewritten into another style (other indentation, other line wrapping, braces on their own line), keeping every token and comment. Much of the corpus is already in google-java-format style, so the first row alone would mostly test leaving formatted code alone. To see the scrambled input for a disagreement:

```bash
cargo xtask scramble path/to/File.java
```

Reference tool versions and corpus commits are pinned in [`scoring.toml`](scoring.toml); downloads go to `.weavejig/` and are checked against the pinned SHA-256. The harness looks for a JDK in `WEAVEJIG_JAVA_HOME`, `JAVA_HOME`, then `PATH`.

The `held-out` set is for measuring only. `cargo xtask score --set held-out` prints totals and never lists individual disagreements: do not inspect its files to fix a score.

The linter is scored rule by rule, and the table of rules is printed under the totals. To work on one rule:

```bash
cargo xtask score --set dev --part linter --rule magic-number --show 50
```

- Checkstyle and PMD are each run with exactly the checks WeaveJig mirrors, with default options (`xtask/src/lint_score.rs` lists the few properties set to match a WeaveJig default). Their output is kept in `.weavejig/cache/` and reused while the checks and the files stay the same; `--no-cache` runs them again.
- A diagnostic matches Checkstyle's if it starts on the same line. PMD often reports the same finding a line or two away, on the statement under an `if` where Checkstyle names the `if`, so a diagnostic matches PMD's if it covers the line PMD reports.
- The `Coverage` rows run Checkstyle's bundled `sun_checks` and `google_checks` and count how many of their diagnostics come from checks with a rule. Under them, `--show` lists the checks without one, most frequent first: what to add next.

### Adding a lint rule

Rules are written from the reference tools' documentation and from what they are seen to do, not from their source.

1. Add a module in `crates/weavejig-linter/src/rules/` with a unit struct implementing `Rule` and a `static META`, built with `RuleMeta::new(id, summary)` and then `.checkstyle(..)` or `.pmd(..)`, `.fixable()`, `.opt_in()` and `.options(..)` as they apply. `crate::ast` has helpers for reading the tree; the rule's `Context` has the file's text, path and lines, and the rule's options.
2. List it in `ALL` in `rules/mod.rs`, which is sorted by id.
3. Add code that trips the rule, and code that nearly does, to a probe in `corpus/smoke/lint/`. Where the documentation leaves a case open, running the reference tool on a probe settles it.
4. Run `cargo xtask score --part linter` and `cargo xtask score --set dev --part linter --rule <id>`, and compare with the reference until they agree.
5. Record what the probes now report (see `corpus/smoke/lint/README.md`), add the rule to the table above, and add cases to `crates/weavejig-linter/tests/rules.rs` for its fix and options.

### Changing the formatter

Formatting rules live in `crates/weavejig-formatter/src/format.rs`; a node kind without a rule is laid out on one line by the spacing table in `spacing.rs`. Long string literals are split after layout, in `strings.rs`. Fixtures in `crates/weavejig-formatter/tests/fixtures/` record current output so that changes show up in review; see the README there.
