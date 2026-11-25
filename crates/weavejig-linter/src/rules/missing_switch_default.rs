use weavejig_syntax::{SyntaxKind::*, SyntaxNode, child_node, child_token};

use crate::{Context, Diagnostic, Rule, RuleMeta};

/// A `switch` statement without a `default` silently does nothing for a
/// value nobody thought of.
///
/// Switch expressions, and switch statements over patterns or with a
/// `case null`, are left alone: the compiler already requires those to cover
/// every value.
pub(crate) struct MissingSwitchDefault;

static META: RuleMeta = RuleMeta::new(
    "missing-switch-default",
    "`switch` statements should have a `default` branch",
)
.checkstyle("MissingSwitchDefault");

impl Rule for MissingSwitchDefault {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if node.kind() != SWITCH_STMT {
            return;
        }
        let Some(block) = child_node(node, SWITCH_BLOCK) else {
            return;
        };
        let labels: Vec<SyntaxNode> = block
            .children()
            .flat_map(|entry| entry.children().filter(|child| child.kind() == CASE_LABEL))
            .collect();
        let has_default = labels
            .iter()
            .any(|label| child_token(label, DEFAULT_KW).is_some());
        // The compiler checks these for exhaustiveness itself.
        let checked_by_compiler = labels.iter().any(|label| {
            label.children().any(|child| {
                matches!(child.kind(), TYPE_PATTERN | RECORD_PATTERN)
                    || (child.kind() == LITERAL && child_token(&child, NULL_KW).is_some())
            })
        });
        if has_default || checked_by_compiler {
            return;
        }
        let Some(keyword) = child_token(node, SWITCH_KW) else {
            return;
        };
        out.push(Diagnostic::new(
            &META,
            keyword.text_range(),
            "`switch` without a `default` branch",
        ));
    }
}
