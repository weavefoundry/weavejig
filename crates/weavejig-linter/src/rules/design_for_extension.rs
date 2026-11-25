use weavejig_syntax::{SyntaxKind::*, SyntaxNode, first_token};

use crate::ast;
use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// A method that subclasses outside the package can override, and that
/// does something, is part of the class's contract with them whether or not
/// that was meant. Either say how to override it safely, in a Javadoc
/// comment, or make it `final`, `abstract` or empty.
///
/// Only classes that can be extended from outside are checked: public or
/// protected ones, all the way out, that are not final and have a
/// constructor a subclass could call.
pub(crate) struct DesignForExtension;

static META: RuleMeta = RuleMeta::new(
    "design-for-extension",
    "overridable methods of public classes should document how to override them, or be final",
)
.checkstyle("DesignForExtension")
.opt_in()
.options(&[
    OptionSpec {
        name: "ignored-annotations",
        default: OptionDefault::List(&["After", "AfterClass", "Before", "BeforeClass", "Test"]),
        help: "annotations whose methods are not checked",
    },
    OptionSpec {
        name: "required-javadoc-phrase",
        default: OptionDefault::Regex(".*"),
        help: "what the method's Javadoc comment must contain to count",
    },
]);

/// Whether a class outside the package can extend `class`.
fn is_extensible(class: &SyntaxNode) -> bool {
    if class.kind() != CLASS_DECL
        || ast::has_modifier(class, FINAL_KW)
        || !ast::is_visible_outside(class)
    {
        return false;
    }
    let members = ast::members(class);
    let mut constructors = members
        .iter()
        .filter(|member| member.kind() == CONSTRUCTOR_DECL)
        .peekable();
    constructors.peek().is_none()
        || constructors.any(|constructor| !ast::has_modifier(constructor, PRIVATE_KW))
}

impl Rule for DesignForExtension {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if node.kind() != METHOD_DECL {
            return;
        }
        let has = |kind| ast::has_modifier(node, kind);
        if has(PRIVATE_KW) || has(STATIC_KW) || has(FINAL_KW) || has(ABSTRACT_KW) {
            return;
        }
        // An empty method does nothing a subclass could break.
        let does_something = match ast::body(node) {
            Some(body) => body.children().next().is_some(),
            None => has(NATIVE_KW),
        };
        if !does_something || !ast::owner(node).is_some_and(|class| is_extensible(&class)) {
            return;
        }
        let ignored = ast::annotations(node).iter().any(|annotation| {
            let name = ast::annotation_name(annotation);
            let simple = name.rsplit('.').next().unwrap_or(&name);
            ctx.options
                .list("ignored-annotations")
                .iter()
                .any(|ignored| ignored == simple)
        });
        let documented = ast::javadoc(node).is_some_and(|comment| {
            ctx.options
                .matches("required-javadoc-phrase", comment.text())
        });
        if ignored || documented {
            return;
        }
        if let Some(first) = first_token(node) {
            out.push(Diagnostic::new(
                &META,
                first.text_range(),
                format!(
                    "`{}` can be overridden; document how to do so safely, or make it `final`",
                    ast::name_text(node)
                ),
            ));
        }
    }
}
