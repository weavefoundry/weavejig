use weavejig_syntax::{SyntaxKind::*, SyntaxNode, first_token};

use crate::ast;
use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// An interface that only holds constants is not a type anyone implements
/// for its behaviour. Constants belong in a class.
pub(crate) struct InterfaceIsType;

static META: RuleMeta = RuleMeta::new(
    "interface-is-type",
    "interfaces should declare methods, not only constants",
)
.checkstyle("InterfaceIsType")
.opt_in()
.options(&[OptionSpec {
    name: "allow-marker-interfaces",
    default: OptionDefault::Bool(true),
    help: "allow interfaces with no members at all",
}]);

impl Rule for InterfaceIsType {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if node.kind() != INTERFACE_DECL {
            return;
        }
        let members = ast::members(node);
        if members.iter().any(|member| member.kind() == METHOD_DECL) {
            return;
        }
        let has_fields = members.iter().any(|member| member.kind() == FIELD_DECL);
        if !has_fields && ctx.options.bool("allow-marker-interfaces") {
            return;
        }
        let Some(first) = first_token(node) else {
            return;
        };
        out.push(Diagnostic::new(
            &META,
            first.text_range(),
            format!("interface `{}` declares no methods", ast::name_text(node)),
        ));
    }
}
