use weavejig_syntax::{SyntaxKind::*, SyntaxNode, child_token};

use crate::{Context, Diagnostic, Rule, RuleMeta};

/// A block that does nothing: the body of an `if`, `else`, loop, `try`,
/// `finally`, `synchronized` or initializer with no statement in it, or a
/// `switch` with no cases. A comment does not make a block less empty.
///
/// `catch` blocks have a rule of their own, `empty-catch-block`, and an
/// empty method body is often right.
pub(crate) struct EmptyBlock;

static META: RuleMeta = RuleMeta::new(
    "empty-block",
    "blocks of control statements and initializers should not be empty",
)
.checkstyle("EmptyBlock")
.pmd("codestyle", "EmptyControlStatement");

impl Rule for EmptyBlock {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let what = match node.kind() {
            SWITCH_BLOCK => match node.parent().map(|parent| parent.kind()) {
                Some(SWITCH_STMT) => "switch",
                _ => return,
            },
            BLOCK => match node.parent().map(|parent| parent.kind()) {
                Some(IF_STMT) => "if",
                Some(WHILE_STMT) => "while",
                Some(DO_STMT) => "do",
                Some(FOR_STMT | FOREACH_STMT) => "for",
                Some(TRY_STMT) => "try",
                Some(FINALLY_CLAUSE) => "finally",
                Some(SYNCHRONIZED_STMT) => "synchronized",
                Some(INITIALIZER_BLOCK) => "initializer",
                _ => return,
            },
            _ => return,
        };
        if node.children().next().is_some() {
            return;
        }
        let Some(open) = child_token(node, L_BRACE) else {
            return;
        };
        let what = match (what, open.prev_sibling_or_token()) {
            // The block after `else` is a child of the `if` statement too.
            ("if", _) if is_else_branch(node) => "else",
            (what, _) => what,
        };
        out.push(Diagnostic::new(
            &META,
            open.text_range(),
            format!("`{what}` block has no statements"),
        ));
    }
}

fn is_else_branch(block: &SyntaxNode) -> bool {
    let mut previous = block.prev_sibling_or_token();
    while let Some(element) = previous {
        if element.kind() == ELSE_KW {
            return true;
        }
        if !element.kind().is_trivia() {
            return false;
        }
        previous = element.prev_sibling_or_token();
    }
    false
}
