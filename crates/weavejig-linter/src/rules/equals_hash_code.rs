use weavejig_syntax::{SyntaxKind::*, SyntaxNode, child_token, first_token};

use crate::ast;
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// Equal objects must have equal hash codes, so a class that overrides
/// `equals` or `hashCode` must override both.
pub(crate) struct EqualsHashCode;

static META: RuleMeta = RuleMeta::new(
    "equals-hash-code",
    "classes that override `equals` or `hashCode` should override both",
)
.checkstyle("EqualsHashCode")
.pmd("errorprone", "OverrideBothEqualsAndHashcode");

/// A method with a body, or a native one: something that defines what the
/// method does.
fn is_defined(method: &SyntaxNode, name: &str) -> bool {
    method.kind() == METHOD_DECL
        && ast::name_text(method) == name
        && (ast::body(method).is_some() || ast::has_modifier(method, NATIVE_KW))
}

/// `equals(Object o)`, where `Object` may be qualified. Whatever its
/// visibility, it is the method collections call.
pub(crate) fn is_equals(method: &SyntaxNode) -> bool {
    if !is_defined(method, "equals") || ast::has_modifier(method, STATIC_KW) {
        return false;
    }
    match ast::params(method).as_slice() {
        [param] => is_object(param),
        _ => false,
    }
}

/// Whether the parameter's type is `Object`, as opposed to an array of them.
pub(crate) fn is_object(param: &SyntaxNode) -> bool {
    ast::declared_type(param).is_some_and(|ty| {
        let text = ast::compact(&ty);
        text == "Object" || text == "java.lang.Object"
    }) && child_token(param, L_BRACK).is_none()
}

fn is_hash_code(method: &SyntaxNode) -> bool {
    is_defined(method, "hashCode") && ast::params(method).is_empty()
}

impl Rule for EqualsHashCode {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if node.kind() != CLASS_BODY {
            return;
        }
        let members: Vec<SyntaxNode> = node.children().collect();
        let equals = members.iter().find(|member| is_equals(member));
        let hash_code = members.iter().find(|member| is_hash_code(member));
        let (method, message) = match (equals, hash_code) {
            (Some(equals), None) => (equals, "`equals` is overridden without `hashCode`"),
            (None, Some(hash_code)) => (hash_code, "`hashCode` is overridden without `equals`"),
            _ => return,
        };
        // The method's header up to its name: Checkstyle reports the first
        // of it, annotations included, and PMD the method itself.
        if let (Some(first), Some(name)) = (first_token(method), ast::name(method)) {
            let header = first.text_range().cover(name.text_range());
            out.push(Diagnostic::new(&META, header, message));
        }
    }
}
