use weavejig_syntax::{SyntaxKind::*, SyntaxNode, child_token};

use crate::ast;
use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// A block directly inside another block, or directly under a `case`: a
/// scope with nothing to say what it is for. Usually the remains of a
/// deleted `if` or loop.
pub(crate) struct AvoidNestedBlocks;

static META: RuleMeta = RuleMeta::new(
    "avoid-nested-blocks",
    "blocks should not be nested directly inside other blocks",
)
.checkstyle("AvoidNestedBlocks")
.options(&[OptionSpec {
    name: "allow-in-switch-case",
    default: OptionDefault::Bool(false),
    help: "allow a block that is all there is to a `case`",
}]);

impl Rule for AvoidNestedBlocks {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if node.kind() != BLOCK {
            return;
        }
        let Some(parent) = node.parent() else {
            return;
        };
        match parent.kind() {
            BLOCK => {}
            SWITCH_GROUP => {
                if ctx.options.bool("allow-in-switch-case") && ast::statements(&parent).len() == 1 {
                    return;
                }
            }
            _ => return,
        }
        let Some(open) = child_token(node, L_BRACE) else {
            return;
        };
        out.push(Diagnostic::new(
            &META,
            open.text_range(),
            "nested block; give it a purpose or remove the braces",
        ));
    }
}
