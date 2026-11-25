use weavejig_syntax::{SyntaxKind::*, SyntaxNode, child_node, child_token, last_token};

use crate::ast;
use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// A `case` whose statements run on into the next `case`, without a comment
/// saying that this is meant: `// fall through`, as the last comment before
/// the next `case`.
pub(crate) struct FallThrough;

static META: RuleMeta = RuleMeta::new(
    "fall-through",
    "a `case` should not fall through to the next without a comment saying so",
)
.checkstyle("FallThrough")
.pmd("errorprone", "ImplicitSwitchFallThrough")
.options(&[
    OptionSpec {
        name: "relief-pattern",
        default: OptionDefault::Regex("falls?[ -]?thr(u|ough)"),
        help: "what a comment must match to allow falling through",
    },
    OptionSpec {
        name: "check-last-case-group",
        default: OptionDefault::Bool(false),
        help: "also require the last group of a `switch` not to run off its end",
    },
]);

impl Rule for FallThrough {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if node.kind() != SWITCH_GROUP {
            return;
        }
        let next = node.next_sibling();
        if next.is_none() && !ctx.options.bool("check-last-case-group") {
            return;
        }
        let Some(last) = ast::statements(node).pop() else {
            return;
        };
        if terminates(&last, true, true) || has_relief_comment(&last, ctx) {
            return;
        }
        // Reported where control arrives: the next label, or failing one,
        // the end of the switch.
        let at = match &next {
            Some(next) => child_node(next, CASE_LABEL).and_then(|label| {
                ast::tokens(&label).find(|token| matches!(token.kind(), CASE_KW | DEFAULT_KW))
            }),
            None => node.parent().and_then(|block| child_token(&block, R_BRACE)),
        };
        let Some(at) = at else {
            return;
        };
        out.push(Diagnostic::new(
            &META,
            at.text_range(),
            "the previous `case` falls through to here",
        ));
    }
}

/// Whether control cannot run past the end of `statement`. `use_break` and
/// `use_continue` say whether a `break` or `continue` leaves the `case`,
/// which it does not inside a nested loop or `switch`.
fn terminates(statement: &SyntaxNode, use_break: bool, use_continue: bool) -> bool {
    let recurse = |node: &SyntaxNode| terminates(node, use_break, use_continue);
    match statement.kind() {
        RETURN_STMT | THROW_STMT | YIELD_STMT => true,
        BREAK_STMT => use_break,
        CONTINUE_STMT => use_continue,
        BLOCK => statement
            .children()
            .last()
            .is_some_and(|last| recurse(&last)),
        LABELED_STMT | SYNCHRONIZED_STMT => statement
            .children()
            .last()
            .is_some_and(|last| recurse(&last)),
        IF_STMT => {
            // Children: the condition, the `then` branch and the `else` one.
            let branches: Vec<SyntaxNode> = statement.children().skip(1).collect();
            branches.len() == 2 && branches.iter().all(recurse)
        }
        WHILE_STMT | DO_STMT | FOR_STMT | FOREACH_STMT => {
            let body = match statement.kind() {
                DO_STMT => statement.children().next(),
                _ => statement.children().last(),
            };
            body.is_some_and(|body| terminates(&body, false, false))
        }
        TRY_STMT => {
            let finally = child_node(statement, FINALLY_CLAUSE).and_then(|f| child_node(&f, BLOCK));
            if finally.is_some_and(|block| recurse(&block)) {
                return true;
            }
            let body = child_node(statement, BLOCK);
            let catches = statement
                .children()
                .filter(|child| child.kind() == CATCH_CLAUSE)
                .filter_map(|clause| child_node(&clause, BLOCK));
            body.is_some_and(|body| recurse(&body))
                && catches.into_iter().all(|block| recurse(&block))
        }
        SWITCH_STMT => {
            let Some(block) = child_node(statement, SWITCH_BLOCK) else {
                return false;
            };
            let mut entries = block.children().peekable();
            if entries.peek().is_none() {
                return false;
            }
            entries.all(|entry| {
                ast::statements(&entry)
                    .pop()
                    .is_some_and(|last| terminates(&last, false, use_continue))
            })
        }
        _ => false,
    }
}

/// Whether the last comment between the group's last statement and the next
/// label says that falling through is meant.
fn has_relief_comment(last: &SyntaxNode, ctx: &Context<'_>) -> bool {
    let mut relief = false;
    let mut token = last_token(last).and_then(|token| token.next_token());
    while let Some(current) = token.filter(|token| token.kind().is_trivia()) {
        if current.kind().is_comment() {
            relief = ctx.options.matches("relief-pattern", current.text());
        }
        token = current.next_token();
    }
    relief
}
