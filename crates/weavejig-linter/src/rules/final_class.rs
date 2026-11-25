use weavejig_syntax::{SyntaxKind::*, SyntaxNode, child_node, first_token};

use crate::ast;
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// A class whose constructors are all private, or a private class with no
/// constructor of its own, cannot be subclassed from outside. Unless
/// something in the file extends it, it is final in all but name, and
/// saying so helps readers and the compiler.
///
/// Checkstyle's `FinalClass` overlooks some subclasses in the same file: an
/// anonymous one written before the class it extends, and one that names
/// its superclass through a generic outer class (`Outer<T>.Inner`). It then
/// asks for `final` on a class that could not compile with it. This rule
/// finds those subclasses, and so reports a little less.
pub(crate) struct FinalClass;

static META: RuleMeta = RuleMeta::new(
    "final-class",
    "classes that cannot be subclassed should be declared `final`",
)
.checkstyle("FinalClass");

impl Rule for FinalClass {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if node.kind() != CLASS_DECL
            || ast::has_modifier(node, FINAL_KW)
            || ast::has_modifier(node, ABSTRACT_KW)
        {
            return;
        }
        let members = ast::members(node);
        let constructors: Vec<&SyntaxNode> = members
            .iter()
            .filter(|member| member.kind() == CONSTRUCTOR_DECL)
            .collect();
        // Nothing outside can call a private constructor, or the default
        // constructor of a private class.
        let unreachable = if constructors.is_empty() {
            ast::has_modifier(node, PRIVATE_KW)
        } else {
            constructors
                .iter()
                .all(|constructor| ast::has_modifier(constructor, PRIVATE_KW))
        };
        if !unreachable {
            return;
        }
        let name = ast::name_text(node);
        if is_extended(ctx.root, node) {
            return;
        }
        let Some(first) = first_token(node) else {
            return;
        };
        out.push(Diagnostic::new(
            &META,
            first.text_range(),
            format!("class `{name}` cannot be subclassed and should be `final`"),
        ));
    }
}

/// Whether something in the file extends `class`: a class declaration, or
/// an anonymous class.
fn is_extended(root: &SyntaxNode, class: &SyntaxNode) -> bool {
    let name = ast::name_text(class);
    root.descendants().any(|node| {
        let superclass = match node.kind() {
            CLASS_DECL => {
                child_node(&node, EXTENDS_CLAUSE).and_then(|clause| ast::declared_type(&clause))
            }
            NEW_EXPR if child_node(&node, CLASS_BODY).is_some() => ast::declared_type(&node),
            _ => None,
        };
        let Some(superclass) = superclass.filter(|ty| ty.kind() == NAMED_TYPE) else {
            return false;
        };
        let parts: Vec<String> = ast::tokens(&superclass)
            .filter(|token| token.kind() == IDENT)
            .map(|token| token.text().to_string())
            .collect();
        match parts.as_slice() {
            [] => false,
            [simple] => {
                *simple == name && resolve(&node, simple).is_some_and(|found| found == *class)
            }
            // `Outer.Inner`: the names of the class and the types around
            // it, as far as the reference spells them out.
            qualified => {
                let mut path: Vec<String> = class
                    .ancestors()
                    .filter(|ancestor| ancestor.kind().is_type_decl())
                    .map(|ancestor| ast::name_text(&ancestor))
                    .collect();
                path.reverse();
                path.ends_with(qualified) || qualified.ends_with(&path)
            }
        }
    })
}

/// The type declared in this file that the simple name `name` means at
/// `site`: the nearest one in scope.
fn resolve(site: &SyntaxNode, name: &str) -> Option<SyntaxNode> {
    let named = |node: &SyntaxNode| node.kind().is_type_decl() && ast::name_text(node) == name;
    for scope in site.ancestors() {
        if named(&scope) && scope != *site {
            return Some(scope);
        }
        let declared = match scope.kind() {
            // Members of a type, local classes of a block, top-level types.
            CLASS_BODY | BLOCK | SWITCH_GROUP | COMPILATION_UNIT => scope.children().find(named),
            _ => None,
        };
        if declared.is_some() {
            return declared;
        }
    }
    None
}
