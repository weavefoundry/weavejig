use weavejig_syntax::{SyntaxKind::*, SyntaxNode, SyntaxToken, first_token};

use crate::{Context, Diagnostic, Rule, RuleMeta};

/// Two statements on one line: the second is easy to miss, and a debugger
/// cannot stop between them.
///
/// The parts of a `for` header are not statements, and neither are the
/// resources of a `try`.
pub(crate) struct OneStatementPerLine;

static META: RuleMeta = RuleMeta::new(
    "one-statement-per-line",
    "each statement should be on a line of its own",
)
.checkstyle("OneStatementPerLine");

/// The statement or declaration that the `;` token ends, if it ends one.
fn ended_by(semi: &SyntaxToken) -> Option<SyntaxNode> {
    let parent = semi.parent()?;
    match parent.kind() {
        EXPR_STMT | RETURN_STMT | THROW_STMT | BREAK_STMT | CONTINUE_STMT | YIELD_STMT
        | ASSERT_STMT | DO_STMT | IMPORT_DECL | PACKAGE_DECL | FIELD_DECL => Some(parent),
        // In a `for` header, the declaration's `;` belongs to the loop.
        LOCAL_VAR_DECL => Some(parent),
        _ => None,
    }
}

impl Rule for OneStatementPerLine {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_file(&self, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        // Where the previous statement ended: the line, and the `;` itself.
        let mut previous_end: Option<u32> = None;
        let mut previous_semi = None;
        let mut token = ctx.root.first_token();
        while let Some(current) = token {
            token = current.next_token();
            if current.kind() != SEMI {
                continue;
            }
            let Some(statement) = ended_by(&current) else {
                continue;
            };
            let Some(first) = first_token(&statement) else {
                continue;
            };
            // A statement inside this one, such as the body of a lambda
            // among its arguments, is part of it and not its neighbour.
            let encloses_previous =
                previous_semi.is_some_and(|semi| statement.text_range().contains(semi));
            if !encloses_previous && previous_end == Some(ctx.line_of(&first)) {
                out.push(Diagnostic::new(
                    &META,
                    current.text_range(),
                    "only one statement per line",
                ));
            }
            previous_end = Some(ctx.line_of(&current));
            previous_semi = Some(current.text_range().start());
        }
    }
}
