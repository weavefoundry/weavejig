use weavejig_syntax::{SyntaxKind::*, SyntaxNode, SyntaxToken};

use crate::ast;
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// `b == true`, `!false`, `b || true`: a boolean literal in a boolean
/// expression is noise, or a mistake.
pub(crate) struct SimplifyBooleanExpression;

static META: RuleMeta = RuleMeta::new(
    "simplify-boolean-expression",
    "boolean expressions should not contain `true` or `false` literals",
)
.checkstyle("SimplifyBooleanExpression");

/// The operator of the boolean expression `literal` is an operand of.
fn operator_over(literal: &SyntaxNode) -> Option<SyntaxToken> {
    let mut operand = literal.clone();
    let mut parent = operand.parent()?;
    while parent.kind() == PAREN_EXPR {
        operand = parent;
        parent = operand.parent()?;
    }
    let operator = ast::tokens(&parent).find(|token| !matches!(token.kind(), L_PAREN | R_PAREN))?;
    match (parent.kind(), operator.kind()) {
        (PREFIX_EXPR, BANG) => Some(operator),
        (BINARY_EXPR, EQ_EQ | BANG_EQ | PIPE_PIPE | AMP_AMP) => Some(operator),
        // `true ? a : b`, and `c ? true : false`. One literal branch on
        // its own, `c ? true : other`, is a matter of taste.
        (CONDITIONAL_EXPR, QUESTION) => {
            let parts: Vec<SyntaxNode> = parent.children().collect();
            let [condition, then, otherwise] = parts.as_slice() else {
                return None;
            };
            let is_boolean = |branch: &SyntaxNode| {
                let inner = ast::unparenthesized(branch);
                ast::is_literal(&inner, TRUE_KW) || ast::is_literal(&inner, FALSE_KW)
            };
            // Reported once, for the first of two literal branches.
            (*condition == operand
                || (*then == operand && is_boolean(then) && is_boolean(otherwise)))
            .then_some(operator)
        }
        _ => None,
    }
}

impl Rule for SimplifyBooleanExpression {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if !ast::is_literal(node, TRUE_KW) && !ast::is_literal(node, FALSE_KW) {
            return;
        }
        if let Some(operator) = operator_over(node) {
            out.push(Diagnostic::new(
                &META,
                operator.text_range(),
                "boolean expression can be simplified",
            ));
        }
    }
}
