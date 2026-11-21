# Smoke corpus

A handful of hand-written Java files that exercise the parser, formatter and
linter. `cargo xtask score` uses them by default, so the scoring harness works
without downloading anything.

The files under `invalid/` are deliberately not valid Java: the parser score
counts agreement with `javac` on rejections as well as acceptances.

These files are test inputs. Do not reformat them.
