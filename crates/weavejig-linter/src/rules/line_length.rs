use crate::options::{OptionDefault, OptionSpec};
use crate::rules::util::expanded_width;
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// Lines longer than the formatter's line width. The formatter wraps what
/// it can; what is left is mostly long strings and comments.
///
/// `package` and `import` lines cannot be wrapped and are not reported.
pub(crate) struct LineLength;

static META: RuleMeta = RuleMeta::new("line-length", "lines should not be too long")
    .checkstyle("LineLength")
    .opt_in()
    .options(&[
        OptionSpec {
            name: "max",
            default: OptionDefault::Int(100),
            help: "the most columns a line may have",
        },
        OptionSpec {
            name: "ignore-pattern",
            default: OptionDefault::Regex("^(package|import) .*"),
            help: "lines that may be any length",
        },
        OptionSpec {
            name: "tab-width",
            default: OptionDefault::Int(8),
            help: "the distance between tab stops, for lines that contain tabs",
        },
    ]);

impl Rule for LineLength {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_file(&self, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let max = usize::try_from(ctx.options.int("max")).unwrap_or(0);
        let tab_width = usize::try_from(ctx.options.int("tab-width")).unwrap_or(0);
        for line in 1..=ctx.line_count() {
            let text = ctx.line_text(line);
            // Most lines are short enough in bytes, let alone in columns.
            if text.len() <= max && !text.contains('\t') {
                continue;
            }
            let width = expanded_width(text, tab_width);
            if width > max && !ctx.options.matches("ignore-pattern", text) {
                out.push(Diagnostic::new(
                    &META,
                    ctx.line_range(line),
                    format!("line is {width} columns wide; the limit is {max}"),
                ));
            }
        }
    }
}
