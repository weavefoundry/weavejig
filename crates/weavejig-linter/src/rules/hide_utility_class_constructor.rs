use weavejig_syntax::{SyntaxKind::*, SyntaxNode, child_node, first_token};

use crate::ast;
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// A class with only static members is not meant to be instantiated, so it
/// should not have a constructor anyone can call: declare a private one.
pub(crate) struct HideUtilityClassConstructor;

static META: RuleMeta = RuleMeta::new(
    "hide-utility-class-constructor",
    "classes with only static members should not have a public or default constructor",
)
.checkstyle("HideUtilityClassConstructor");

impl Rule for HideUtilityClassConstructor {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        // A static nested class is left alone, as Checkstyle leaves it.
        if node.kind() != CLASS_DECL
            || ast::has_modifier(node, ABSTRACT_KW)
            || ast::has_modifier(node, STATIC_KW)
            || child_node(node, EXTENDS_CLAUSE).is_some()
        {
            return;
        }
        let members = ast::members(node);
        let is_static = |member: &SyntaxNode| ast::has_modifier(member, STATIC_KW);
        let is_member = |member: &&SyntaxNode| matches!(member.kind(), FIELD_DECL | METHOD_DECL);
        let has_instance_member = members.iter().filter(is_member).any(|m| !is_static(m));
        let has_visible_static_member = members
            .iter()
            .filter(is_member)
            .any(|m| is_static(m) && !ast::has_modifier(m, PRIVATE_KW));
        let constructors: Vec<&SyntaxNode> = members
            .iter()
            .filter(|member| member.kind() == CONSTRUCTOR_DECL)
            .collect();
        let callable = constructors.is_empty()
            || constructors.iter().any(|constructor| {
                !ast::has_modifier(constructor, PRIVATE_KW)
                    && !ast::has_modifier(constructor, PROTECTED_KW)
            });
        if has_instance_member || !has_visible_static_member || !callable {
            return;
        }
        let Some(first) = first_token(node) else {
            return;
        };
        out.push(Diagnostic::new(
            &META,
            first.text_range(),
            format!(
                "utility class `{}` should not have a public or default constructor",
                ast::name_text(node)
            ),
        ));
    }
}
