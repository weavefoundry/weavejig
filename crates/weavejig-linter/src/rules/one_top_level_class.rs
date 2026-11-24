use weavejig_syntax::{SyntaxKind::*, SyntaxNode, first_token};

use crate::ast;
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// One top-level type per file: a second one is hard to find, since its
/// name is not the file's.
pub(crate) struct OneTopLevelClass;

static META: RuleMeta = RuleMeta::new(
    "one-top-level-class",
    "each top-level type should be in a file of its own",
)
.checkstyle("OneTopLevelClass");

impl Rule for OneTopLevelClass {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_file(&self, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let types: Vec<SyntaxNode> = ctx
            .root
            .children()
            .filter(|child| child.kind().is_type_decl())
            .collect();
        if types.len() < 2 {
            return;
        }
        // The file belongs to its public type, or failing one, to its first.
        let main = types
            .iter()
            .position(|ty| ast::has_modifier(ty, PUBLIC_KW))
            .unwrap_or(0);
        for (i, ty) in types.iter().enumerate() {
            let Some(first) = first_token(ty).filter(|_| i != main) else {
                continue;
            };
            out.push(Diagnostic::new(
                &META,
                first.text_range(),
                format!(
                    "top-level type `{}` should be in a file of its own",
                    ast::name_text(ty)
                ),
            ));
        }
    }
}
