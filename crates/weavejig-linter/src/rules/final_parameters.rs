use weavejig_syntax::{SyntaxKind::*, SyntaxNode, child_token, first_token};

use crate::ast;
use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Edit, Rule, RuleMeta};

/// Parameters declared `final` cannot be reassigned, so a reader knows that
/// a parameter still holds the caller's value.
///
/// Parameters of methods without a body are left alone: there is nothing
/// that could reassign them. The fix is not offered for a parameter that
/// the body assigns.
pub(crate) struct FinalParameters;

static META: RuleMeta = RuleMeta::new(
    "final-parameters",
    "parameters of methods and constructors should be `final`",
)
.checkstyle("FinalParameters")
.opt_in()
.fixable()
.options(&[OptionSpec {
    name: "ignore-primitive-types",
    default: OptionDefault::Bool(false),
    help: "do not require `final` on parameters of primitive types",
}]);

impl Rule for FinalParameters {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if !matches!(node.kind(), METHOD_DECL | CONSTRUCTOR_DECL) {
            return;
        }
        let Some(body) = ast::body(node) else {
            return;
        };
        for param in ast::params(node) {
            // The receiver parameter, `Foo this`, is not a variable.
            if ast::has_modifier(&param, FINAL_KW) || child_token(&param, THIS_KW).is_some() {
                continue;
            }
            let Some(name) = ast::name(&param).filter(|name| name.text() != "_") else {
                continue;
            };
            let ty = ast::declared_type(&param);
            if ctx.options.bool("ignore-primitive-types")
                && ty.as_ref().is_some_and(|ty| ty.kind() == PRIMITIVE_TYPE)
                && child_token(&param, L_BRACK).is_none()
                && child_token(&param, ELLIPSIS).is_none()
            {
                continue;
            }
            let (Some(first), Some(ty)) = (first_token(&param), ty) else {
                continue;
            };
            let diagnostic = Diagnostic::new(
                &META,
                first.text_range(),
                format!("parameter `{}` should be `final`", name.text()),
            );
            // A parameter the body assigns cannot simply be made final:
            // the code would no longer compile.
            if assigns(&body, name.text()) {
                out.push(diagnostic);
                continue;
            }
            // After any annotations, before the type.
            let at = ty.text_range().start();
            out.push(diagnostic.with_fix("add `final`", vec![Edit::insert(at, "final ")]));
        }
    }
}

/// Whether anything in `body` assigns to, increments or decrements a
/// variable called `name`. A local variable of the same name counts too:
/// the fix is only offered where it is certain to be safe.
fn assigns(body: &SyntaxNode, name: &str) -> bool {
    body.descendants().any(|node| {
        let target = match node.kind() {
            ASSIGN_EXPR => node.children().next(),
            PREFIX_EXPR | POSTFIX_EXPR if matches!(ast::operator(&node).as_str(), "++" | "--") => {
                node.children().next()
            }
            _ => None,
        };
        target.is_some_and(|target| ast::compact(&ast::unparenthesized(&target)) == name)
    })
}
