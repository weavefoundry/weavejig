use weavejig_syntax::{TextRange, TextSize};

use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// A very long file usually holds a class that does too much.
pub(crate) struct FileLength;

static META: RuleMeta = RuleMeta::new("file-length", "files should not be too long")
    .checkstyle("FileLength")
    .opt_in()
    .options(&[OptionSpec {
        name: "max",
        default: OptionDefault::Int(2000),
        help: "the most lines a file may have",
    }]);

impl Rule for FileLength {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_file(&self, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let (lines, max) = (i64::from(ctx.line_count()), ctx.options.int("max"));
        if lines > max {
            out.push(Diagnostic::new(
                &META,
                TextRange::empty(TextSize::new(0)),
                format!("file is {lines} lines long; the limit is {max}"),
            ));
        }
    }
}
