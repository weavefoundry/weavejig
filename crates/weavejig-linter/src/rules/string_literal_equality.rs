use weavejig_syntax::{SyntaxKind::*, SyntaxNode};

use crate::ast;
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// `s == "text"` compares references, not characters. It works only when
/// both strings happen to be interned; `equals` is what was meant.
pub(crate) struct StringLiteralEquality;

static META: RuleMeta = RuleMeta::new(
    "string-literal-equality",
    "string literals should be compared with `equals`, not `==` or `!=`",
)
.checkstyle("StringLiteralEquality");

/// A string literal, or a concatenation with one in it.
fn is_string(expr: &SyntaxNode) -> bool {
    let expr = ast::unparenthesized(expr);
    match expr.kind() {
        LITERAL => ast::is_literal(&expr, STRING_LITERAL) || ast::is_literal(&expr, TEXT_BLOCK),
        BINARY_EXPR if ast::operator(&expr) == "+" => {
            expr.children().any(|operand| is_string(&operand))
        }
        _ => false,
    }
}

impl Rule for StringLiteralEquality {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if node.kind() != BINARY_EXPR {
            return;
        }
        let Some(operator) =
            ast::tokens(node).find(|token| matches!(token.kind(), EQ_EQ | BANG_EQ))
        else {
            return;
        };
        if node.children().any(|operand| is_string(&operand)) {
            out.push(Diagnostic::new(
                &META,
                operator.text_range(),
                format!(
                    "string literal compared with `{}`; use `equals`",
                    operator.text()
                ),
            ));
        }
    }
}
