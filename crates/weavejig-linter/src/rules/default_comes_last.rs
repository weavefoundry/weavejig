use weavejig_syntax::{SyntaxKind::*, SyntaxNode, child_token};

use crate::{Context, Diagnostic, Rule, RuleMeta};

/// `default` is easiest to find, and reads as "everything else", when it is
/// the last branch of a `switch`.
pub(crate) struct DefaultComesLast;

static META: RuleMeta = RuleMeta::new(
    "default-comes-last",
    "`default` should be the last branch of a `switch`",
)
.checkstyle("DefaultComesLast")
.pmd("bestpractices", "DefaultLabelNotLastInSwitch");

impl Rule for DefaultComesLast {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if node.kind() != SWITCH_BLOCK {
            return;
        }
        // `default` may share the last group with `case` labels, in any
        // order: what matters is that no other branch comes after its own.
        let entries: Vec<SyntaxNode> = node.children().collect();
        let Some((_, earlier)) = entries.split_last() else {
            return;
        };
        for label in earlier
            .iter()
            .flat_map(|entry| entry.children().filter(|child| child.kind() == CASE_LABEL))
        {
            if let Some(default) = child_token(&label, DEFAULT_KW) {
                out.push(Diagnostic::new(
                    &META,
                    default.text_range(),
                    "`default` should be the last branch of the `switch`",
                ));
            }
        }
    }
}
