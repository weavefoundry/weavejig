# Lint probes

Hand-written Java that breaks each lint rule, and comes close to breaking it,
in as many ways as could be thought of. The files are scored like the rest of
the smoke corpus (`cargo xtask score`), which compares what WeaveJig reports
for them with what Checkstyle and PMD report.

`diagnostics.txt` lists what WeaveJig reports for these files, with every
rule switched on. The linter's tests check that it still does, so a change in
behaviour shows up without a JDK at hand. After an intended change, score the
smoke corpus to see that the reference tools agree, then regenerate the list:

```bash
WEAVEJIG_UPDATE_FIXTURES=1 cargo test -p weavejig-linter --test probes
```

These files are test inputs. Do not reformat them: `Layout.java` has tabs, a
long line and no final newline on purpose.
