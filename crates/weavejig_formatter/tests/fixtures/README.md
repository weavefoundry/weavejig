# Formatter fixtures

Each `NAME.input.java` is formatted and compared with `NAME.expected.java`.

The expected files record what WeaveJig produces **today**, so that changes in
behaviour show up in review. They have not been checked against
google-java-format, and are not a statement of what the right output is: the
scoring harness (`cargo xtask score`) is what measures that.

After an intended change, regenerate them and review the diff:

```bash
WEAVEJIG_UPDATE_FIXTURES=1 cargo test -p weavejig_formatter --test fixtures
```
