use weavejig_syntax::{TextRange, TextSize};

use crate::{Context, Diagnostic, Edit, Rule, RuleMeta};

/// A file should end with a line terminator: tools that read lines, and
/// `diff`, treat a last line without one as incomplete.
pub(crate) struct NewlineAtEndOfFile;

static META: RuleMeta = RuleMeta::new(
    "newline-at-end-of-file",
    "files should end with a line terminator",
)
.checkstyle("NewlineAtEndOfFile")
.fixable();

impl Rule for NewlineAtEndOfFile {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_file(&self, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if ctx.text.is_empty() || ctx.text.ends_with(['\n', '\r']) {
            return;
        }
        // The file's own terminator, if it has used one.
        let terminator = if ctx.text.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let end = TextSize::try_from(ctx.text.len()).unwrap_or_default();
        // Reported on the first line, as a property of the file.
        out.push(
            Diagnostic::new(
                &META,
                TextRange::empty(TextSize::new(0)),
                "file does not end with a line terminator",
            )
            .with_fix("add a line terminator", vec![Edit::insert(end, terminator)]),
        );
    }
}
