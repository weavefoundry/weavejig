use std::collections::HashSet;

use weavejig_syntax::{SyntaxKind::*, SyntaxNode, first_token};

use crate::ast;
use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// The parts of a class in the order the Java code conventions give:
/// static variables, instance variables, constructors, methods; and the
/// variables of each kind from most to least visible (public, protected,
/// package, private).
pub(crate) struct DeclarationOrder;

static META: RuleMeta = RuleMeta::new(
    "declaration-order",
    "class members should be ordered: static variables, instance variables, constructors, methods",
)
.checkstyle("DeclarationOrder")
.opt_in()
.options(&[
    OptionSpec {
        name: "ignore-constructors",
        default: OptionDefault::Bool(false),
        help: "do not check where constructors are",
    },
    OptionSpec {
        name: "ignore-modifiers",
        default: OptionDefault::Bool(false),
        help: "do not check the order of variables by `static` and by visibility",
    },
]);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Section {
    StaticVariables,
    InstanceVariables,
    Constructors,
    Methods,
}

/// Visibility, most visible first.
fn access(member: &SyntaxNode) -> u8 {
    if ast::has_modifier(member, PUBLIC_KW) {
        0
    } else if ast::has_modifier(member, PROTECTED_KW) {
        1
    } else if ast::has_modifier(member, PRIVATE_KW) {
        3
    } else {
        2
    }
}

/// Whether the field's initializer mentions a name among `fields`: those
/// declared so far, this one included. Its place may then be forced by
/// what it is computed from.
fn mentions_field(field: &SyntaxNode, fields: &HashSet<String>) -> bool {
    field
        .children()
        .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        .flat_map(|declarator| declarator.children())
        .flat_map(|value| value.descendants_with_tokens())
        .filter_map(|element| element.into_token())
        .any(|token| token.kind() == IDENT && fields.contains(token.text()))
}

impl Rule for DeclarationOrder {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if node.kind() != CLASS_BODY {
            return;
        }
        let ignore_modifiers = ctx.options.bool("ignore-modifiers");
        let in_annotation_type = node
            .parent()
            .is_some_and(|owner| owner.kind() == ANNOTATION_TYPE_DECL);
        let mut section = Section::StaticVariables;
        // The least visible access seen so far in the current section.
        let mut section_access = 0;
        let mut fields: HashSet<String> = HashSet::new();
        for member in node.children() {
            let problem = match member.kind() {
                CONSTRUCTOR_DECL => {
                    if section > Section::Constructors {
                        (!ctx.options.bool("ignore-constructors"))
                            .then_some("constructor should come before the methods")
                    } else {
                        section = Section::Constructors;
                        None
                    }
                }
                // The elements of an annotation type are not methods.
                METHOD_DECL if !in_annotation_type => {
                    section = Section::Methods;
                    None
                }
                FIELD_DECL => {
                    let is_static = ast::has_modifier(&member, STATIC_KW);
                    let mut problem = None;
                    if is_static {
                        if section > Section::StaticVariables
                            && (!ignore_modifiers || section > Section::InstanceVariables)
                        {
                            problem = Some(
                                "static variable should come before instance variables, constructors and methods",
                            );
                        }
                    } else if section > Section::InstanceVariables {
                        problem =
                            Some("instance variable should come before constructors and methods");
                    } else if section == Section::StaticVariables {
                        section = Section::InstanceVariables;
                        section_access = 0;
                    }
                    fields.extend(
                        member
                            .children()
                            .filter(|child| child.kind() == VARIABLE_DECLARATOR)
                            .map(|declarator| ast::name_text(&declarator)),
                    );
                    // Visibility is tracked even for a variable that is in
                    // the wrong section, but only reported for one that is
                    // in the right one.
                    let access = access(&member);
                    if access < section_access {
                        if problem.is_none()
                            && !ignore_modifiers
                            && !mentions_field(&member, &fields)
                        {
                            problem = Some(
                                "variable should come before the less visible variables of its kind",
                            );
                        }
                    } else {
                        section_access = access;
                    }
                    problem
                }
                _ => None,
            };
            if let (Some(problem), Some(first)) = (problem, first_token(&member)) {
                out.push(Diagnostic::new(&META, first.text_range(), problem));
            }
        }
    }
}
