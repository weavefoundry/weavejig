use weavejig_syntax::{SyntaxKind::*, SyntaxNode};

use crate::{Diagnostic, Edit, Fix, Rule, RuleMeta};

/// A lone `;` is an empty statement. It is usually a typo, and as the body of
/// an `if` or loop it is often a bug.
pub(crate) struct EmptyStatement;

static META: RuleMeta = RuleMeta {
    id: "empty-statement",
    summary: "empty statements (a lone `;`) should be removed",
    checkstyle: Some("EmptyStatement"),
    pmd: None,
    fixable: true,
};

impl Rule for EmptyStatement {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        if node.kind() != EMPTY_STMT {
            return;
        }
        // Only an empty statement in a statement list can simply be deleted.
        // As the body of an `if` or loop, removing it would change which
        // statement the construct controls.
        let in_list = node
            .parent()
            .is_some_and(|p| matches!(p.kind(), BLOCK | SWITCH_GROUP));
        out.push(Diagnostic {
            rule: META.id,
            message: "empty statement".to_string(),
            range: node.text_range(),
            fix: in_list.then(|| Fix {
                description: "remove the empty statement".to_string(),
                edits: vec![Edit::delete(node.text_range())],
            }),
        });
    }
}
