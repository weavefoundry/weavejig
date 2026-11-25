//! Rules about `finalize` and `clone`, which share their shape.

use weavejig_syntax::{SyntaxKind::*, SyntaxNode};

use crate::ast;
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// Whether `method` is a declaration of the no-argument method `name`.
fn declares(method: &SyntaxNode, name: &str) -> bool {
    method.kind() == METHOD_DECL && ast::name_text(method) == name && ast::params(method).is_empty()
}

/// Whether `method` calls `super.name()` anywhere in its body.
fn calls_super(method: &SyntaxNode, name: &str) -> bool {
    method.descendants().any(|node| {
        ast::call(&node).is_some_and(|(called, arguments)| {
            called.text() == name
                && ast::arguments(&arguments).is_empty()
                && ast::receiver(&node).is_some_and(|receiver| receiver.kind() == SUPER_EXPR)
        })
    })
}

/// Reports an override of `name` that has a body and never calls the method
/// it overrides.
fn check_calls_super(
    meta: &'static RuleMeta,
    name: &str,
    node: &SyntaxNode,
    out: &mut Vec<Diagnostic>,
) {
    if !declares(node, name) || ast::body(node).is_none() || calls_super(node, name) {
        return;
    }
    if let Some(token) = ast::name(node) {
        out.push(Diagnostic::new(
            meta,
            token.text_range(),
            format!("`{name}` should call `super.{name}()`"),
        ));
    }
}

/// An override of `finalize` that does not call `super.finalize()` skips
/// the cleanup of the classes above it.
pub(crate) struct SuperFinalize;

static SUPER_FINALIZE: RuleMeta = RuleMeta::new(
    "super-finalize",
    "`finalize` should call `super.finalize()`",
)
.checkstyle("SuperFinalize");

impl Rule for SuperFinalize {
    fn meta(&self) -> &'static RuleMeta {
        &SUPER_FINALIZE
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        check_calls_super(&SUPER_FINALIZE, "finalize", node, out);
    }
}

/// An override of `clone` that does not call `super.clone()` returns an
/// object of the wrong class for every subclass.
pub(crate) struct SuperClone;

static SUPER_CLONE: RuleMeta =
    RuleMeta::new("super-clone", "`clone` should call `super.clone()`").checkstyle("SuperClone");

impl Rule for SuperClone {
    fn meta(&self) -> &'static RuleMeta {
        &SUPER_CLONE
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        check_calls_super(&SUPER_CLONE, "clone", node, out);
    }
}

/// Finalizers run late, or never, on a thread of the collector's choosing,
/// and are deprecated for removal. Use `try`-with-resources or a `Cleaner`.
pub(crate) struct NoFinalizer;

static NO_FINALIZER: RuleMeta =
    RuleMeta::new("no-finalizer", "classes should not define `finalize`").checkstyle("NoFinalizer");

impl Rule for NoFinalizer {
    fn meta(&self) -> &'static RuleMeta {
        &NO_FINALIZER
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if !declares(node, "finalize") {
            return;
        }
        if let Some(first) = weavejig_syntax::first_token(node) {
            out.push(Diagnostic::new(
                &NO_FINALIZER,
                first.text_range(),
                "avoid `finalize`: it may run late or never",
            ));
        }
    }
}

/// `clone` and `Cloneable` are easy to get wrong. A copy constructor or a
/// static factory says what a copy is.
pub(crate) struct NoClone;

static NO_CLONE: RuleMeta = RuleMeta::new("no-clone", "classes should not define `clone`")
    .checkstyle("NoClone")
    .opt_in();

impl Rule for NoClone {
    fn meta(&self) -> &'static RuleMeta {
        &NO_CLONE
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if !declares(node, "clone") {
            return;
        }
        if let Some(first) = weavejig_syntax::first_token(node) {
            out.push(Diagnostic::new(
                &NO_CLONE,
                first.text_range(),
                "avoid `clone`: prefer a copy constructor or a factory method",
            ));
        }
    }
}
