use weavejig_syntax::{
    SyntaxKind::{self, *},
    SyntaxNode, SyntaxToken, TextRange, child_node,
};

use crate::ast;
use crate::{Context, Diagnostic, Edit, Rule, RuleMeta};

/// A modifier that says what the language already says:
///
/// - `public` and `abstract` on interface methods; `public`, `static` and
///   `final` on interface fields; `public` and `static` on types nested in
///   an interface;
/// - `abstract` on an interface; `static` on a nested interface, enum or
///   record; `final` on a record;
/// - `final` on a method that cannot be overridden anyway: a private one,
///   one of a final or anonymous class, or a static one of an enum;
/// - `final` on the parameters of a method without a body, on the
///   resources of a `try`, and on unnamed variables (`_`);
/// - a visibility modifier on an enum constructor, and `public` on the
///   constructor of a class or record that cannot be seen from outside its
///   package;
/// - `strictfp`, which has done nothing since Java 17.
///
/// Checkstyle does not count the members of an annotation type as public,
/// and so calls `public` redundant on the constructor of a class nested in
/// one. Removing it there would change what the constructor is visible to,
/// so this rule does not.
pub(crate) struct RedundantModifier;

static META: RuleMeta = RuleMeta::new(
    "redundant-modifier",
    "modifiers that the language already implies should be removed",
)
.checkstyle("RedundantModifier")
.fixable();

fn report(modifier: &SyntaxToken, why: &str, out: &mut Vec<Diagnostic>) {
    // The modifier goes with the space after it.
    let mut end = modifier.text_range().end();
    if let Some(space) = modifier
        .next_token()
        .filter(|next| next.kind() == WHITESPACE)
        && !space.text().contains(['\n', '\r'])
    {
        end = space.text_range().end();
    }
    let removal = TextRange::new(modifier.text_range().start(), end);
    out.push(
        Diagnostic::new(
            &META,
            modifier.text_range(),
            format!("redundant `{}`: {why}", modifier.text()),
        )
        .with_fix(
            format!("remove `{}`", modifier.text()),
            vec![Edit::delete(removal)],
        ),
    );
}

/// Reports each of `kinds` that `node` has.
fn check(node: &SyntaxNode, kinds: &[SyntaxKind], why: &str, out: &mut Vec<Diagnostic>) {
    for kind in kinds {
        if let Some(modifier) = ast::modifier(node, *kind) {
            report(&modifier, why, out);
        }
    }
}

/// The type `member` is declared in, and whether that is an anonymous class
/// or the body of an enum constant.
fn owner_of(member: &SyntaxNode) -> Option<(SyntaxNode, bool)> {
    let owner = ast::owner(member)?;
    let anonymous = matches!(owner.kind(), NEW_EXPR | ENUM_CONSTANT);
    Some((owner, anonymous))
}

impl Rule for RedundantModifier {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let kind = node.kind();
        if kind == RESOURCE {
            check(node, &[FINAL_KW], "a resource is implicitly final", out);
            return;
        }
        // `final` on a variable that has no name to assign to.
        let unnamed = match kind {
            LOCAL_VAR_DECL => node
                .children()
                .filter(|child| child.kind() == VARIABLE_DECLARATOR)
                .all(|declarator| ast::name_text(&declarator) == "_"),
            PARAM | TYPE_PATTERN => ast::name_text(node) == "_",
            _ => false,
        };
        if unnamed {
            check(
                node,
                &[FINAL_KW],
                "an unnamed variable cannot be assigned",
                out,
            );
            return;
        }
        if !kind.is_type_decl() && !matches!(kind, METHOD_DECL | FIELD_DECL | CONSTRUCTOR_DECL) {
            return;
        }
        let owner = owner_of(node);
        let in_interface = owner
            .as_ref()
            .is_some_and(|(owner, _)| ast::is_interface(owner));

        if kind != FIELD_DECL && kind != CONSTRUCTOR_DECL {
            check(
                node,
                &[STRICTFP_KW],
                "it has had no effect since Java 17",
                out,
            );
        }
        match kind {
            METHOD_DECL if in_interface => {
                check(
                    node,
                    &[PUBLIC_KW, ABSTRACT_KW],
                    "interface methods are implicitly public and, without a body, abstract",
                    out,
                );
            }
            FIELD_DECL if in_interface => {
                check(
                    node,
                    &[PUBLIC_KW, STATIC_KW, FINAL_KW],
                    "interface fields are implicitly public, static and final",
                    out,
                );
            }
            _ if kind.is_type_decl() && in_interface => {
                check(
                    node,
                    &[PUBLIC_KW, STATIC_KW],
                    "types nested in an interface are implicitly public and static",
                    out,
                );
            }
            _ => {}
        }
        match kind {
            INTERFACE_DECL | ANNOTATION_TYPE_DECL => {
                check(
                    node,
                    &[ABSTRACT_KW],
                    "interfaces are implicitly abstract",
                    out,
                );
                if !in_interface {
                    check(
                        node,
                        &[STATIC_KW],
                        "nested interfaces are implicitly static",
                        out,
                    );
                }
            }
            ENUM_DECL if !in_interface => {
                check(
                    node,
                    &[STATIC_KW],
                    "nested enums are implicitly static",
                    out,
                );
            }
            RECORD_DECL => {
                check(node, &[FINAL_KW], "records are implicitly final", out);
                if !in_interface {
                    check(
                        node,
                        &[STATIC_KW],
                        "nested records are implicitly static",
                        out,
                    );
                }
            }
            METHOD_DECL => {
                if !in_interface {
                    self.final_method(node, owner.as_ref(), out);
                }
                if ast::body(node).is_none() {
                    for param in ast::params(node) {
                        check(
                            &param,
                            &[FINAL_KW],
                            "the parameter of a method without a body cannot be assigned",
                            out,
                        );
                    }
                }
            }
            CONSTRUCTOR_DECL => {
                let Some((owner, _)) = owner else {
                    return;
                };
                if owner.kind() == ENUM_DECL {
                    check(
                        node,
                        &[PRIVATE_KW, PUBLIC_KW, PROTECTED_KW],
                        "enum constructors are implicitly private",
                        out,
                    );
                } else if matches!(owner.kind(), CLASS_DECL | RECORD_DECL)
                    // The compact constructor of a record is left alone.
                    && child_node(node, PARAM_LIST).is_some()
                    && !ast::is_visible_outside(&owner)
                {
                    check(
                        node,
                        &[PUBLIC_KW],
                        "the class is not public, so its constructor cannot be more visible",
                        out,
                    );
                }
            }
            _ => {}
        }
    }
}

impl RedundantModifier {
    /// `final` on a method that nothing could override.
    fn final_method(
        &self,
        method: &SyntaxNode,
        owner: Option<&(SyntaxNode, bool)>,
        out: &mut Vec<Diagnostic>,
    ) {
        // `@SafeVarargs` needs the method to be final or static.
        if ast::has_annotation(method, "SafeVarargs") {
            return;
        }
        let why = if ast::has_modifier(method, PRIVATE_KW) {
            "a private method cannot be overridden"
        } else {
            match owner {
                Some((_, true)) => "a method of an anonymous class cannot be overridden",
                Some((owner, false)) if ast::has_modifier(owner, FINAL_KW) => "the class is final",
                // The constants of an enum can override its methods, but
                // not its static ones.
                Some((owner, false))
                    if owner.kind() == ENUM_DECL && ast::has_modifier(method, STATIC_KW) =>
                {
                    "a static method of an enum cannot be overridden"
                }
                _ => return,
            }
        };
        check(method, &[FINAL_KW], why, out);
    }
}
