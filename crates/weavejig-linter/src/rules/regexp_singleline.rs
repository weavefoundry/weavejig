use weavejig_syntax::{TextRange, TextSize};

use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// A house rule written as a regular expression: every line that matches
/// `format` is reported. Use it for what has no rule of its own, such as
/// trailing spaces (`\s+$`) or a forbidden call (`System\.out\.println`).
///
/// With the default `format`, which matches nothing, the rule does nothing.
pub(crate) struct RegexpSingleline;

static META: RuleMeta = RuleMeta::new(
    "regexp-singleline",
    "lines should not match the configured pattern",
)
.checkstyle("RegexpSingleline")
.options(&[
    OptionSpec {
        name: "format",
        default: OptionDefault::Regex("$."),
        help: "what a line must not match",
    },
    OptionSpec {
        name: "message",
        default: OptionDefault::Str(""),
        help: "what to say about a line that matches",
    },
]);

impl Rule for RegexpSingleline {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_file(&self, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let Some(regex) = ctx.options.regex("format") else {
            return;
        };
        // The default cannot match, so there is nothing to look for.
        if regex.as_str() == "$." {
            return;
        }
        for line in 1..=ctx.line_count() {
            let range = ctx.line_range(line);
            let Some(found) = regex.find(&ctx.text[range]) else {
                continue;
            };
            let start = range.start() + TextSize::try_from(found.start()).unwrap_or_default();
            let len = TextSize::try_from(found.len()).unwrap_or_default();
            let message = match ctx.options.str("message") {
                "" => format!("line matches the forbidden pattern `{}`", regex.as_str()),
                message => message.to_string(),
            };
            out.push(Diagnostic::new(&META, TextRange::at(start, len), message));
        }
    }
}
