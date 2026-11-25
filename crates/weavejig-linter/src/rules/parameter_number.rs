use weavejig_syntax::{SyntaxKind::*, SyntaxNode};

use crate::ast;
use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// A method with many parameters is hard to call correctly. Group them in
/// an object, or split the method.
pub(crate) struct ParameterNumber;

static META: RuleMeta = RuleMeta::new(
    "parameter-number",
    "methods and constructors should not have too many parameters",
)
.checkstyle("ParameterNumber")
.opt_in()
.options(&[
    OptionSpec {
        name: "max",
        default: OptionDefault::Int(7),
        help: "the most parameters a method or constructor may have",
    },
    OptionSpec {
        name: "ignore-overridden-methods",
        default: OptionDefault::Bool(false),
        help: "do not check methods annotated `@Override`, whose parameters are not theirs to choose",
    },
]);

impl Rule for ParameterNumber {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if !matches!(node.kind(), METHOD_DECL | CONSTRUCTOR_DECL) {
            return;
        }
        let count = ast::params(node).len() as i64;
        let max = ctx.options.int("max");
        if count <= max
            || (ctx.options.bool("ignore-overridden-methods")
                && ast::has_annotation(node, "Override"))
        {
            return;
        }
        if let Some(name) = ast::name(node) {
            out.push(Diagnostic::new(
                &META,
                name.text_range(),
                format!(
                    "`{}` has {count} parameters; the limit is {max}",
                    name.text()
                ),
            ));
        }
    }
}
