use weavejig_syntax::{SyntaxKind::*, SyntaxNode, first_token};

use crate::{Context, Diagnostic, Rule, RuleMeta};

/// One variable per declaration, and one declaration per line: `int a, b;`
/// and `int a; int b;` both make a variable easy to overlook.
///
/// The variables of a `for` loop header are left alone.
pub(crate) struct MultipleVariableDeclarations;

static META: RuleMeta = RuleMeta::new(
    "multiple-variable-declarations",
    "each variable should be declared in its own statement, on its own line",
)
.checkstyle("MultipleVariableDeclarations")
.pmd("bestpractices", "OneDeclarationPerLine");

fn is_declaration(node: &SyntaxNode) -> bool {
    matches!(node.kind(), FIELD_DECL | LOCAL_VAR_DECL)
        && !node
            .parent()
            .is_some_and(|parent| matches!(parent.kind(), FOR_STMT | FOREACH_STMT))
}

impl Rule for MultipleVariableDeclarations {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if !is_declaration(node) {
            return;
        }
        let Some(first) = first_token(node) else {
            return;
        };
        let declarators = node
            .children()
            .filter(|child| child.kind() == VARIABLE_DECLARATOR)
            .count();
        if declarators > 1 {
            out.push(Diagnostic::new(
                &META,
                first.text_range(),
                "each variable should be declared in its own statement",
            ));
        }
        let Some(next) = node.next_sibling().filter(is_declaration) else {
            return;
        };
        if ctx.first_line(&next) == ctx.last_line(node)
            && let Some(next_first) = first_token(&next)
        {
            out.push(Diagnostic::new(
                &META,
                next_first.text_range(),
                "only one variable declaration per line",
            ));
        }
    }
}
