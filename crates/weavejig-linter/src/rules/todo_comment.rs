use weavejig_syntax::SyntaxToken;

use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// A `TODO:` comment is work that was put off. This rule lists them.
pub(crate) struct TodoComment;

static META: RuleMeta = RuleMeta::new("todo-comment", "comments should not contain `TODO:`")
    .checkstyle("TodoComment")
    .opt_in()
    .options(&[OptionSpec {
        name: "format",
        default: OptionDefault::Regex("TODO:"),
        help: "what a comment must not match",
    }]);

impl Rule for TodoComment {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_file(&self, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let mut token: Option<SyntaxToken> = ctx.root.first_token();
        while let Some(current) = token {
            token = current.next_token();
            if !current.kind().is_comment() {
                continue;
            }
            // Each line of a block comment is looked at on its own, so that
            // `^` and `$` in the format mean a line's ends.
            if current
                .text()
                .lines()
                .any(|line| ctx.options.matches("format", line))
            {
                out.push(Diagnostic::new(
                    &META,
                    current.text_range(),
                    "comment matches the to-do format",
                ));
            }
        }
    }
}
