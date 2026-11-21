use weavejig_syntax::{SyntaxKind::*, SyntaxNode, SyntaxToken, TextRange, child_token};

use crate::{Diagnostic, Edit, Fix, Rule, RuleMeta};

/// Array brackets belong on the type (`int[] a`), not the name (`int a[]`).
pub(crate) struct ArrayTypeStyle;

static META: RuleMeta = RuleMeta {
    id: "array-type-style",
    summary: "array brackets should be on the type, not the variable name",
    checkstyle: Some("ArrayTypeStyle"),
    pmd: None,
    fixable: true,
};

impl Rule for ArrayTypeStyle {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_token(&self, token: &SyntaxToken, out: &mut Vec<Diagnostic>) {
        if token.kind() != L_BRACK {
            return;
        }
        // Brackets that are direct children of these nodes follow a name; in
        // a type they would be inside an `ARRAY_TYPE`.
        let Some(parent) = token.parent() else {
            return;
        };
        if !matches!(parent.kind(), VARIABLE_DECLARATOR | PARAM | METHOD_DECL) {
            return;
        }
        let Some(close) = token.next_sibling_or_token().and_then(|el| el.into_token()) else {
            return;
        };
        if close.kind() != R_BRACK {
            return;
        }
        let brackets = token.text_range().cover(close.text_range());
        out.push(Diagnostic {
            rule: META.id,
            message: "array brackets should be on the type, not the name".to_string(),
            range: brackets,
            fix: fix(&parent, brackets),
        });
    }
}

/// Moves one pair of brackets to the end of the declared type. Offered only
/// where that cannot change the type of another variable.
fn fix(parent: &SyntaxNode, brackets: TextRange) -> Option<Fix> {
    let owner = match parent.kind() {
        PARAM if child_token(parent, ELLIPSIS).is_none() => parent.clone(),
        VARIABLE_DECLARATOR => {
            let decl = parent.parent()?;
            let declarators = decl
                .children()
                .filter(|n| n.kind() == VARIABLE_DECLARATOR)
                .count();
            if declarators != 1 {
                return None;
            }
            decl
        }
        _ => return None,
    };
    let ty = owner
        .children()
        .find(|n| matches!(n.kind(), PRIMITIVE_TYPE | NAMED_TYPE | ARRAY_TYPE))?;
    Some(Fix {
        description: "move the brackets to the type".to_string(),
        edits: vec![
            Edit::insert(ty.text_range().end(), "[]"),
            Edit::delete(brackets),
        ],
    })
}
