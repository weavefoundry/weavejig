use weavejig_syntax::{SyntaxKind::*, SyntaxNode, child_token};

use crate::ast;
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// `if (c) return true; else return false;` is `return c;`.
pub(crate) struct SimplifyBooleanReturn;

static META: RuleMeta = RuleMeta::new(
    "simplify-boolean-return",
    "an `if` that only returns `true` or `false` should return its condition",
)
.checkstyle("SimplifyBooleanReturn");

/// Whether `branch` is `return true;` or `return false;`, alone or as the
/// only statement of a block.
fn returns_boolean_literal(branch: &SyntaxNode) -> bool {
    let statement = match branch.kind() {
        BLOCK => {
            let mut statements = branch.children();
            match (statements.next(), statements.next()) {
                (Some(only), None) => only,
                _ => return false,
            }
        }
        _ => branch.clone(),
    };
    statement.kind() == RETURN_STMT
        && statement.children().next().is_some_and(|value| {
            ast::is_literal(&value, TRUE_KW) || ast::is_literal(&value, FALSE_KW)
        })
}

impl Rule for SimplifyBooleanReturn {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if node.kind() != IF_STMT {
            return;
        }
        // Children: the condition, the `then` branch and the `else` one.
        let branches: Vec<SyntaxNode> = node.children().skip(1).collect();
        if branches.len() != 2 || !branches.iter().all(returns_boolean_literal) {
            return;
        }
        if let Some(keyword) = child_token(node, IF_KW) {
            out.push(Diagnostic::new(
                &META,
                keyword.text_range(),
                "conditional logic can be removed: return the condition instead",
            ));
        }
    }
}
