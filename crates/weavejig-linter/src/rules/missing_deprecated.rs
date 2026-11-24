use weavejig_syntax::{SyntaxKind::*, SyntaxNode, child_token, first_token};

use crate::ast;
use crate::javadoc;
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// Deprecation is said twice: `@Deprecated` for the compiler, and a
/// `@deprecated` Javadoc tag for people, saying what to use instead. A
/// documented declaration with one should have the other.
pub(crate) struct MissingDeprecated;

static META: RuleMeta = RuleMeta::new(
    "missing-deprecated",
    "`@Deprecated` and the `@deprecated` Javadoc tag should be used together",
)
.checkstyle("MissingDeprecated");

impl Rule for MissingDeprecated {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let declares = node.kind().is_type_decl()
            || matches!(
                node.kind(),
                METHOD_DECL | CONSTRUCTOR_DECL | FIELD_DECL | ENUM_CONSTANT | PACKAGE_DECL
            );
        if !declares {
            return;
        }
        // Without a Javadoc comment there is nowhere for the tag to go.
        let Some(comment) = ast::javadoc(node) else {
            return;
        };
        let annotated = ast::has_annotation(node, "Deprecated");
        let tagged = javadoc::has_block_tag(comment.text(), "@deprecated");
        if annotated == tagged {
            return;
        }
        // A package is reported at `package`, after its annotations.
        let at = match node.kind() {
            PACKAGE_DECL => child_token(node, PACKAGE_KW),
            _ => first_token(node),
        };
        let Some(first) = at else {
            return;
        };
        let message = if annotated {
            "`@Deprecated` declaration should have a `@deprecated` Javadoc tag saying what to use instead"
        } else {
            "declaration with a `@deprecated` Javadoc tag should be annotated `@Deprecated`"
        };
        out.push(Diagnostic::new(&META, first.text_range(), message));
    }
}
