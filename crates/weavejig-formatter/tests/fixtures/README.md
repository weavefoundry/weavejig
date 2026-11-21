# Formatter fixtures

Each `NAME.input.java` is formatted and compared with `NAME.expected.java`.

The expected files record what WeaveJig produces **today**, so that changes in
behaviour show up in review. They have not been checked against
google-java-format, and are not a statement of what the right output is: the
scoring harness (`cargo xtask score`) is what measures that.

After an intended change, regenerate them and review the diff:

```bash
WEAVEJIG_UPDATE_FIXTURES=1 cargo test -p weavejig-formatter --test fixtures
```

## Reference fixtures

`reference/NAME.input.java` is formatted and compared with
`reference/NAME.expected.java`, which is the output of google-java-format
2.0.0 on that input. These are small hand-written probes of one behaviour
each, and they do state what the right output is. Never regenerate them from
WeaveJig; to add one, run google-java-format on the new input:

```bash
java -jar .weavejig/reference/google-java-format-2.0.0-all-deps.jar - \
  < reference/NAME.input.java > reference/NAME.expected.java
```
