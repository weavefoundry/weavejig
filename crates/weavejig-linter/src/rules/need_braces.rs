use weavejig_syntax::{SyntaxKind, SyntaxKind::*, SyntaxNode, SyntaxToken, child_token};

use crate::{Diagnostic, Edit, Fix, Rule, RuleMeta};

/// The bodies of `if`, `else`, `for`, `while` and `do` must be blocks.
pub(crate) struct NeedBraces;

static META: RuleMeta = RuleMeta {
    id: "need-braces",
    summary: "`if`, `else`, `for`, `while` and `do` bodies should use braces",
    checkstyle: Some("NeedBraces"),
    pmd: None,
    fixable: true,
};

impl Rule for NeedBraces {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        let children: Vec<SyntaxNode> = node.children().collect();
        match node.kind() {
            IF_STMT => {
                // Children: condition, then-branch, optional else-branch.
                check_body(node, IF_KW, children.get(1), out);
                // `else if` chains are fine without braces around the `if`.
                if let Some(otherwise) = children.get(2).filter(|n| n.kind() != IF_STMT) {
                    check_body(node, ELSE_KW, Some(otherwise), out);
                }
            }
            WHILE_STMT => check_body(node, WHILE_KW, children.get(1), out),
            DO_STMT => check_body(node, DO_KW, children.first(), out),
            FOR_STMT | FOREACH_STMT => check_body(node, FOR_KW, children.last(), out),
            _ => {}
        }
    }
}

fn check_body(
    stmt: &SyntaxNode,
    keyword: SyntaxKind,
    body: Option<&SyntaxNode>,
    out: &mut Vec<Diagnostic>,
) {
    let (Some(body), Some(keyword)) = (body, child_token(stmt, keyword)) else {
        return;
    };
    if body.kind() == BLOCK {
        return;
    }
    out.push(Diagnostic {
        rule: META.id,
        message: format!("`{}` body should use braces", keyword.text()),
        range: keyword.text_range(),
        fix: Some(fix(&keyword, body)),
    });
}

fn fix(keyword: &SyntaxToken, body: &SyntaxNode) -> Fix {
    let range = body.text_range();
    let edits = if body.kind() == EMPTY_STMT {
        // `while (x);` has no space to reuse before the new block.
        let spaced = body
            .first_token()
            .and_then(|t| t.prev_token())
            .is_some_and(|t| t.kind().is_trivia());
        vec![Edit::replace(range, if spaced { "{}" } else { " {}" })]
    } else {
        vec![
            Edit::insert(range.start(), "{ "),
            Edit::insert(range.end(), " }"),
        ]
    };
    Fix {
        description: format!("add braces to the `{}` body", keyword.text()),
        edits,
    }
}
