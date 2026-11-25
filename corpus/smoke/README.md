# Smoke corpus

A handful of hand-written Java files that exercise the parser, formatter and
linter. `cargo xtask score` uses them by default, so the scoring harness works
without downloading anything.

The files under `invalid/` are deliberately not valid Java: the parser score
counts agreement with `javac` on rejections as well as acceptances.

`lint/` holds probes written for the lint rules: code that breaks each rule,
and code that nearly does. See the README there.

These files are test inputs. Do not reformat them.
