use weavejig_syntax::{SyntaxKind::*, SyntaxNode, first_token};

use crate::ast;
use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// A method too long to see at once is too long to understand at once.
pub(crate) struct MethodLength;

static META: RuleMeta = RuleMeta::new(
    "method-length",
    "methods and constructors should not be too long",
)
.checkstyle("MethodLength")
.opt_in()
.options(&[
    OptionSpec {
        name: "max",
        default: OptionDefault::Int(150),
        help: "the most lines a method or constructor may have, from its `{` to its `}`",
    },
    OptionSpec {
        name: "count-empty",
        default: OptionDefault::Bool(true),
        help: "count blank lines and lines with only a comment",
    },
]);

impl Rule for MethodLength {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if !matches!(node.kind(), METHOD_DECL | CONSTRUCTOR_DECL) {
            return;
        }
        let Some(body) = ast::body(node) else {
            return;
        };
        let (first, last) = (ctx.first_line(&body), ctx.last_line(&body));
        let mut length = i64::from(last - first + 1);
        if !ctx.options.bool("count-empty") {
            // Lines of the body on which no code starts or continues.
            let mut code_lines = std::collections::HashSet::new();
            for token in body
                .descendants_with_tokens()
                .filter_map(|element| element.into_token())
                .filter(|token| !token.kind().is_trivia())
            {
                let range = token.text_range();
                code_lines.extend(ctx.line(range.start())..=ctx.line(range.end()));
            }
            length = code_lines.len() as i64;
        }
        let max = ctx.options.int("max");
        if length <= max {
            return;
        }
        if let Some(first) = first_token(node) {
            out.push(Diagnostic::new(
                &META,
                first.text_range(),
                format!(
                    "`{}` is {length} lines long; the limit is {max}",
                    ast::name_text(node)
                ),
            ));
        }
    }
}
