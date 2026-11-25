use weavejig_syntax::{SyntaxKind::*, SyntaxNode};

use crate::ast;
use crate::rules::equals_hash_code::is_object;
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// `equals(Foo other)` overloads `equals` instead of overriding it:
/// collections call `equals(Object)`, which is still the inherited one.
pub(crate) struct CovariantEquals;

static META: RuleMeta = RuleMeta::new(
    "covariant-equals",
    "classes that define `equals` for their own type should also override `equals(Object)`",
)
.checkstyle("CovariantEquals");

impl Rule for CovariantEquals {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        // Interfaces cannot say what `equals(Object)` does, and the body of
        // an enum constant is left alone, as Checkstyle leaves it.
        if node.kind() != CLASS_BODY
            || node
                .parent()
                .is_none_or(|owner| ast::is_interface(&owner) || owner.kind() == ENUM_CONSTANT)
        {
            return;
        }
        // Each instance method `equals` with one parameter, and whether the
        // parameter is an `Object`.
        let equals: Vec<(SyntaxNode, bool)> = node
            .children()
            .filter(|member| {
                member.kind() == METHOD_DECL
                    && ast::name_text(member) == "equals"
                    && !ast::has_modifier(member, STATIC_KW)
            })
            .filter_map(|method| match ast::params(&method).as_slice() {
                [param] => Some((method.clone(), is_object(param))),
                _ => None,
            })
            .collect();
        if equals.iter().any(|(_, takes_object)| *takes_object) {
            return;
        }
        for (method, _) in equals {
            // Without a body there is nothing that could be called instead.
            if ast::body(&method).is_none() {
                continue;
            }
            if let Some(name) = ast::name(&method) {
                out.push(Diagnostic::new(
                    &META,
                    name.text_range(),
                    "`equals` is overloaded for a type without overriding `equals(Object)`",
                ));
            }
        }
    }
}
