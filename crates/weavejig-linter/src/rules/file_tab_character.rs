use weavejig_syntax::{TextRange, TextSize};

use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// Tab characters look different in every editor. Indent with spaces.
pub(crate) struct FileTabCharacter;

static META: RuleMeta = RuleMeta::new(
    "file-tab-character",
    "files should not contain tab characters",
)
.checkstyle("FileTabCharacter")
.opt_in()
.options(&[OptionSpec {
    name: "each-line",
    default: OptionDefault::Bool(false),
    help: "report every line with a tab, not only the first",
}]);

impl Rule for FileTabCharacter {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_file(&self, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if !ctx.text.contains('\t') {
            return;
        }
        let each_line = ctx.options.bool("each-line");
        for line in 1..=ctx.line_count() {
            let range = ctx.line_range(line);
            let Some(column) = ctx.text[range].find('\t') else {
                continue;
            };
            let at = range.start() + TextSize::try_from(column).unwrap_or_default();
            let message = if each_line {
                "line contains a tab character"
            } else {
                "file contains tab characters (this is the first)"
            };
            out.push(Diagnostic::new(
                &META,
                TextRange::at(at, TextSize::new(1)),
                message,
            ));
            if !each_line {
                return;
            }
        }
    }
}
