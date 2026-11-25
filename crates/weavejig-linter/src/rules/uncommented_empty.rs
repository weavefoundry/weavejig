//! Empty bodies that do not say why they are empty.

use weavejig_syntax::{SyntaxKind::*, SyntaxNode, child_token};

use crate::ast;
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// The block of `node`, if it has neither a statement nor a comment in it.
fn silent_empty_body(node: &SyntaxNode) -> Option<SyntaxNode> {
    let body = ast::body(node)?;
    let has_content = body.children().next().is_some()
        || body
            .children_with_tokens()
            .any(|element| element.kind().is_comment());
    (!has_content).then_some(body)
}

/// An empty method is either unfinished or deliberately does nothing. A
/// comment in its body tells the two apart.
pub(crate) struct UncommentedEmptyMethodBody;

static METHOD: RuleMeta = RuleMeta::new(
    "uncommented-empty-method-body",
    "empty methods should have a comment saying why they are empty",
)
.pmd("documentation", "UncommentedEmptyMethodBody")
.opt_in();

impl Rule for UncommentedEmptyMethodBody {
    fn meta(&self) -> &'static RuleMeta {
        &METHOD
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if node.kind() != METHOD_DECL {
            return;
        }
        if let Some(open) = silent_empty_body(node).and_then(|body| child_token(&body, L_BRACE)) {
            out.push(Diagnostic::new(
                &METHOD,
                open.text_range(),
                format!(
                    "method `{}` is empty; add a comment saying why",
                    ast::name_text(node)
                ),
            ));
        }
    }
}

/// An empty constructor that is written out is there for a reason: to
/// narrow its visibility, or for a framework. A comment says which.
///
/// A private constructor is left alone, since hiding it is reason enough,
/// and so is one annotated for injection.
pub(crate) struct UncommentedEmptyConstructor;

static CONSTRUCTOR: RuleMeta = RuleMeta::new(
    "uncommented-empty-constructor",
    "empty constructors should have a comment saying why they are there",
)
.pmd("documentation", "UncommentedEmptyConstructor")
.opt_in();

impl Rule for UncommentedEmptyConstructor {
    fn meta(&self) -> &'static RuleMeta {
        &CONSTRUCTOR
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if node.kind() != CONSTRUCTOR_DECL
            || ast::has_modifier(node, PRIVATE_KW)
            || ast::has_annotation(node, "Inject")
            || ast::has_annotation(node, "Autowired")
        {
            return;
        }
        if let Some(open) = silent_empty_body(node).and_then(|body| child_token(&body, L_BRACE)) {
            out.push(Diagnostic::new(
                &CONSTRUCTOR,
                // The constructor from its first token, where PMD reports
                // it, to the brace that opens its empty body.
                weavejig_syntax::first_token(node).map_or(open.text_range(), |first| {
                    first.text_range().cover(open.text_range())
                }),
                format!(
                    "constructor `{}` is empty; add a comment saying why",
                    ast::name_text(node)
                ),
            ));
        }
    }
}
