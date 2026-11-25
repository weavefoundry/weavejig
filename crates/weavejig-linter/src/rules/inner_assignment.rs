use weavejig_syntax::{SyntaxKind::*, SyntaxNode};

use crate::ast;
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// An assignment buried in an expression, as in `if ((a = next()) > 0)` or
/// `x = y = 0`, is easy to read past, or to mistake for `==`.
///
/// Allowed: an assignment that is a statement of its own, the parts of a
/// `for` header, the value of a lambda, and the loop idiom
/// `while ((line = reader.readLine()) != null)`.
pub(crate) struct InnerAssignment;

static META: RuleMeta = RuleMeta::new(
    "inner-assignment",
    "assignments should not be buried in other expressions",
)
.checkstyle("InnerAssignment");

fn is_comparison(expr: &SyntaxNode) -> bool {
    expr.kind() == BINARY_EXPR
        && matches!(
            ast::operator(expr).as_str(),
            "==" | "!=" | "<" | ">" | "<=" | ">="
        )
}

/// Whether the assignment is where one may stand.
fn is_allowed(assignment: &SyntaxNode) -> bool {
    let Some(parent) = assignment.parent() else {
        return true;
    };
    match parent.kind() {
        // `a = b;`, but not as the value of a switch expression's rule.
        EXPR_STMT => {
            return !parent
                .parent()
                .filter(|rule| rule.kind() == SWITCH_RULE)
                .and_then(|rule| rule.parent())
                .and_then(|block| block.parent())
                .is_some_and(|switch| switch.kind() == SWITCH_EXPR);
        }
        // `for (i = 0; ...; i += 2)`, `x -> a = x`, and a resource.
        FOR_STMT | LAMBDA_EXPR | RESOURCE => return true,
        _ => {}
    }
    // `while ((a = b) != c)`: the assignment, in parentheses, is one side
    // of a comparison in the condition of a loop. The comparison may be
    // joined to others with `&&` and `||`, and to nothing else.
    let mut operand = parent;
    if operand.kind() != PAREN_EXPR {
        return false;
    }
    while let Some(outer) = operand.parent().filter(|outer| outer.kind() == PAREN_EXPR) {
        operand = outer;
    }
    let Some(mut condition) = operand.parent().filter(is_comparison) else {
        return false;
    };
    while let Some(outer) = condition.parent().filter(|outer| {
        outer.kind() == PAREN_EXPR
            || (outer.kind() == BINARY_EXPR && matches!(ast::operator(outer).as_str(), "&&" | "||"))
    }) {
        condition = outer;
    }
    condition
        .parent()
        .is_some_and(|statement| matches!(statement.kind(), WHILE_STMT | DO_STMT | FOR_STMT))
}

impl Rule for InnerAssignment {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if node.kind() != ASSIGN_EXPR || is_allowed(node) {
            return;
        }
        if let Some(operator) = ast::tokens(node).find(|token| token.kind().is_assignment_op()) {
            out.push(Diagnostic::new(
                &META,
                operator.text_range(),
                "assignment inside another expression",
            ));
        }
    }
}
