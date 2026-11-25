use weavejig_syntax::{SyntaxKind::*, SyntaxNode, first_token};

use crate::ast;
use crate::javadoc;
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// A method documented with `{@inheritDoc}` overrides something, so it
/// should say so with `@Override`, and let the compiler check it.
pub(crate) struct MissingOverride;

static META: RuleMeta = RuleMeta::new(
    "missing-override",
    "methods documented with `{@inheritDoc}` should be annotated `@Override`",
)
.checkstyle("MissingOverride");

impl Rule for MissingOverride {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if node.kind() != METHOD_DECL || ast::has_annotation(node, "Override") {
            return;
        }
        let inherits = ast::javadoc(node)
            .is_some_and(|comment| javadoc::has_inline_tag(comment.text(), "inheritDoc"));
        let Some(first) = first_token(node).filter(|_| inherits) else {
            return;
        };
        // A static or private method overrides nothing, so there the tag
        // itself is the mistake.
        let message = if ast::has_modifier(node, STATIC_KW) || ast::has_modifier(node, PRIVATE_KW) {
            "`{@inheritDoc}` is used on a method that cannot override anything"
        } else {
            "method documented with `{@inheritDoc}` should be annotated `@Override`"
        };
        out.push(Diagnostic::new(&META, first.text_range(), message));
    }
}
