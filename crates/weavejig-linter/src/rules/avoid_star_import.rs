use weavejig_syntax::{SyntaxKind::*, SyntaxNode, child_node, child_token};

use crate::{Diagnostic, Rule, RuleMeta};

/// Wildcard imports hide where names come from and can change meaning when a
/// dependency adds a class.
pub(crate) struct AvoidStarImport;

static META: RuleMeta = RuleMeta {
    id: "avoid-star-import",
    summary: "imports should name a single type or member, not `*`",
    checkstyle: Some("AvoidStarImport"),
    pmd: None,
    fixable: false,
};

impl Rule for AvoidStarImport {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        if node.kind() != IMPORT_DECL || child_token(node, STAR).is_none() {
            return;
        }
        let name = child_node(node, NAME)
            .map(|name| name.text().to_string())
            .unwrap_or_default();
        out.push(Diagnostic {
            rule: META.id,
            message: format!("wildcard import `{name}.*` should be avoided"),
            range: node.text_range(),
            fix: None,
        });
    }
}
