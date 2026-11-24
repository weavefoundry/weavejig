use std::collections::HashSet;

use weavejig_syntax::{SyntaxKind::*, SyntaxNode, SyntaxToken, child_node};

use crate::ast;
use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// A local variable or parameter with the name of a field of the class it
/// is in. Inside its scope the bare name means the variable, and code that
/// meant the field silently uses the wrong one.
///
/// In a static context only static fields can be hidden.
pub(crate) struct HiddenField;

static META: RuleMeta = RuleMeta::new(
    "hidden-field",
    "local variables and parameters should not have the name of a field",
)
.checkstyle("HiddenField")
.opt_in()
.options(&[
    OptionSpec {
        name: "ignore-constructor-parameter",
        default: OptionDefault::Bool(false),
        help: "allow constructor parameters to have the names of the fields they set",
    },
    OptionSpec {
        name: "ignore-setter",
        default: OptionDefault::Bool(false),
        help: "allow the parameter of a setter (`void setX(T x)`) to have the name of its field",
    },
    OptionSpec {
        name: "setter-can-return-its-class",
        default: OptionDefault::Bool(false),
        help: "with `ignore-setter`, also treat methods that return their own class as setters",
    },
    OptionSpec {
        name: "ignore-abstract-methods",
        default: OptionDefault::Bool(false),
        help: "allow the parameters of methods without a body to have the names of fields",
    },
    OptionSpec {
        name: "ignore-format",
        default: OptionDefault::Regex("^$"),
        help: "names that may hide fields",
    },
]);

/// The fields of one type declaration.
struct Frame {
    /// The class, enum or record, or the enum constant with a body.
    owner: SyntaxNode,
    statics: HashSet<String>,
    instances: HashSet<String>,
}

fn frame_of(owner: &SyntaxNode) -> Frame {
    let mut frame = Frame {
        owner: owner.clone(),
        statics: HashSet::new(),
        instances: HashSet::new(),
    };
    for member in ast::members(owner) {
        if member.kind() != FIELD_DECL {
            continue;
        }
        let names = member
            .children()
            .filter(|child| child.kind() == VARIABLE_DECLARATOR)
            .map(|declarator| ast::name_text(&declarator));
        if ast::has_modifier(&member, STATIC_KW) {
            frame.statics.extend(names);
        } else {
            frame.instances.extend(names);
        }
    }
    // The components of a record are its fields.
    if owner.kind() == RECORD_DECL {
        frame
            .instances
            .extend(ast::params(owner).iter().map(ast::name_text));
    }
    frame
}

fn is_frame(node: &SyntaxNode) -> bool {
    matches!(node.kind(), CLASS_DECL | ENUM_DECL | RECORD_DECL)
        || (node.kind() == ENUM_CONSTANT && child_node(node, CLASS_BODY).is_some())
}

/// Whether `binding` is in a static context, where instance fields cannot
/// be used and so cannot be hidden. As Checkstyle has it, the nearest
/// enclosing method or variable declaration decides, wherever the classes
/// in between begin and end; the methods of anonymous classes are passed
/// over.
fn in_static_context(binding: &SyntaxNode) -> bool {
    for ancestor in binding.ancestors().skip(1) {
        match ancestor.kind() {
            INITIALIZER_BLOCK if ast::has_modifier(&ancestor, STATIC_KW) => return true,
            METHOD_DECL => {
                let in_anonymous = ast::owner(&ancestor)
                    .is_some_and(|owner| matches!(owner.kind(), NEW_EXPR | ENUM_CONSTANT));
                if !in_anonymous {
                    return ast::has_modifier(&ancestor, STATIC_KW);
                }
            }
            FIELD_DECL | LOCAL_VAR_DECL => return ast::has_modifier(&ancestor, STATIC_KW),
            _ => {}
        }
    }
    false
}

/// Whether `name`, declared at `binding`, is the name of a field it hides.
/// `in_static` says that only static fields count.
fn hides_field(binding: &SyntaxNode, name: &str, in_static: bool) -> bool {
    let mut instance_visible = !in_static;
    for ancestor in binding.ancestors().skip(1).filter(is_frame) {
        let frame = frame_of(&ancestor);
        if frame.statics.contains(name) || (instance_visible && frame.instances.contains(name)) {
            return true;
        }
        // A static nested type, which a record always is, cannot see the
        // instance fields of the types around it.
        instance_visible &=
            !ast::has_modifier(&frame.owner, STATIC_KW) && frame.owner.kind() != RECORD_DECL;
    }
    false
}

/// Whether `method` is a setter for `name`: `void setName(T name)`, or with
/// `returns_own_class`, one returning the class it is declared in.
fn is_setter(method: &SyntaxNode, name: &str, returns_own_class: bool) -> bool {
    let method_name = ast::name_text(method);
    let Some(property) = method_name.strip_prefix("set") else {
        return false;
    };
    let mut expected = String::new();
    let mut chars = name.chars();
    if let Some(first) = chars.next() {
        expected.extend(first.to_uppercase());
        expected.push_str(chars.as_str());
    }
    if property != expected || ast::params(method).len() != 1 {
        return false;
    }
    let Some(ty) = ast::declared_type(method) else {
        return false;
    };
    let returns = ast::compact(&ty);
    returns == "void"
        || (returns_own_class
            && ast::owner(method).is_some_and(|owner| ast::name_text(&owner) == returns))
}

impl HiddenField {
    fn check(
        &self,
        binding: &SyntaxNode,
        name: &SyntaxToken,
        ctx: &Context<'_>,
        out: &mut Vec<Diagnostic>,
    ) {
        let text = name.text();
        let in_static = in_static_context(binding);
        if ctx.options.matches("ignore-format", text) || !hides_field(binding, text, in_static) {
            return;
        }
        // The parameter's owner, for the exemptions.
        if binding.kind() == PARAM
            && let Some(owner) = binding.parent().and_then(|list| list.parent())
        {
            let options = ctx.options;
            let exempt = match owner.kind() {
                CONSTRUCTOR_DECL => options.bool("ignore-constructor-parameter"),
                METHOD_DECL => {
                    (options.bool("ignore-setter")
                        && is_setter(&owner, text, options.bool("setter-can-return-its-class")))
                        || (options.bool("ignore-abstract-methods") && ast::body(&owner).is_none())
                }
                _ => false,
            };
            if exempt {
                return;
            }
        }
        out.push(Diagnostic::new(
            &META,
            name.text_range(),
            format!("`{text}` hides a field"),
        ));
    }
}

impl Rule for HiddenField {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if !matches!(node.kind(), LOCAL_VAR_DECL | PARAM | TYPE_PATTERN) {
            return;
        }
        // Nothing declared in an interface is checked: its fields are
        // constants, which a variable is not mistaken for.
        if ast::enclosing_type(node).is_some_and(|ty| ast::is_interface(&ty)) {
            return;
        }
        match node.kind() {
            LOCAL_VAR_DECL => {
                for declarator in node.children().filter(|c| c.kind() == VARIABLE_DECLARATOR) {
                    if let Some(name) = ast::name(&declarator) {
                        self.check(node, &name, ctx, out);
                    }
                }
            }
            // The components of a record are its fields, not variables
            // that hide them.
            PARAM
                if node
                    .parent()
                    .and_then(|list| list.parent())
                    .is_some_and(|owner| owner.kind() == RECORD_DECL) => {}
            _ => {
                if let Some(name) = ast::name(node) {
                    self.check(node, &name, ctx, out);
                }
            }
        }
    }
}
