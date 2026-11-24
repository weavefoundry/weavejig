use weavejig_syntax::{SyntaxKind::*, SyntaxNode, child_node, child_token};

use crate::ast;
use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// A `catch` block with nothing in it swallows the exception without a
/// trace. A comment saying why is enough to make it deliberate.
pub(crate) struct EmptyCatchBlock;

static META: RuleMeta = RuleMeta::new(
    "empty-catch-block",
    "`catch` blocks should handle the exception or say why not",
)
.checkstyle("EmptyCatchBlock")
.options(&[
    OptionSpec {
        name: "exception-variable-name",
        default: OptionDefault::Regex("^$"),
        help: "names of exception variables whose `catch` block may be empty",
    },
    OptionSpec {
        name: "comment-format",
        default: OptionDefault::Regex(".*"),
        help: "what the first line of the block's first comment must match for the block to be allowed",
    },
]);

impl Rule for EmptyCatchBlock {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if node.kind() != CATCH_CLAUSE {
            return;
        }
        let Some(block) = child_node(node, BLOCK) else {
            return;
        };
        if block.children().next().is_some() {
            return;
        }
        let variable = child_node(node, PARAM).map(|param| ast::name_text(&param));
        if variable.is_some_and(|name| ctx.options.matches("exception-variable-name", &name)) {
            return;
        }
        let comment = block
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| token.kind().is_comment());
        if let Some(comment) = comment {
            let text = comment.text();
            let body = text
                .strip_prefix("//")
                .or_else(|| text.strip_prefix("/*"))
                .unwrap_or(text);
            let first_line = body.lines().next().unwrap_or("").trim_end_matches("*/");
            if ctx.options.matches("comment-format", first_line) {
                return;
            }
        }
        let Some(open) = child_token(&block, L_BRACE) else {
            return;
        };
        out.push(Diagnostic::new(
            &META,
            open.text_range(),
            "empty `catch` block",
        ));
    }
}
