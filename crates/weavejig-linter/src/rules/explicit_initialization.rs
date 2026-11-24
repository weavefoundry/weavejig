use weavejig_syntax::{SyntaxKind::*, SyntaxNode, TextRange, child_token};

use crate::ast;
use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Edit, Rule, RuleMeta};

/// A field set to the value it would have anyway: `int n = 0;`,
/// `Object o = null;`, `boolean b = false;`. The initializer does nothing.
pub(crate) struct ExplicitInitialization;

static META: RuleMeta = RuleMeta::new(
    "explicit-initialization",
    "fields should not be explicitly initialized to their type's default value",
)
.checkstyle("ExplicitInitialization")
.pmd("performance", "RedundantFieldInitializer")
.fixable()
.options(&[OptionSpec {
    name: "only-object-references",
    default: OptionDefault::Bool(false),
    help: "report only `null` initializers, not those of primitive types",
}]);

/// Whether a numeric literal is zero: `0`, `0L`, `0x0`, `0.0f`, `0e0`.
fn is_zero(literal: &str) -> bool {
    let text = literal.to_ascii_lowercase().replace('_', "");
    let digits = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0b")) {
        Some(digits) => digits.split('p').next().unwrap_or(digits),
        None => text.split('e').next().unwrap_or(&text),
    };
    let digits = digits.trim_end_matches(['l', 'f', 'd']);
    !digits.is_empty() && digits.chars().all(|c| c == '0' || c == '.')
}

impl Rule for ExplicitInitialization {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        // A final field has no default: it must be given a value.
        if node.kind() != FIELD_DECL
            || ast::has_modifier(node, FINAL_KW)
            || ast::owner(node).is_none_or(|owner| ast::is_interface(&owner))
        {
            return;
        }
        let Some(ty) = ast::declared_type(node) else {
            return;
        };
        for declarator in node
            .children()
            .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        {
            let (Some(name), Some(value)) = (ast::name(&declarator), declarator.children().last())
            else {
                continue;
            };
            let Some(literal) = ast::literal(&value) else {
                continue;
            };
            // `int a[]` is an array, whatever the declared type says.
            let primitive = (ty.kind() == PRIMITIVE_TYPE
                && child_token(&declarator, L_BRACK).is_none())
            .then(|| ast::compact(&ty));
            let is_default = match primitive.as_deref() {
                None => literal.kind() == NULL_KW,
                Some(_) if ctx.options.bool("only-object-references") => false,
                Some("boolean") => literal.kind() == FALSE_KW,
                Some("char") => {
                    literal.text() == "'\\0'"
                        || (literal.kind() == INT_LITERAL && is_zero(literal.text()))
                }
                Some(_) => {
                    matches!(literal.kind(), INT_LITERAL | FLOAT_LITERAL) && is_zero(literal.text())
                }
            };
            if !is_default {
                continue;
            }
            let Some(before_eq) = child_token(&declarator, EQ)
                .and_then(|eq| weavejig_syntax::prev_significant_token(&eq))
            else {
                continue;
            };
            let initializer =
                TextRange::new(before_eq.text_range().end(), value.text_range().end());
            out.push(
                Diagnostic::new(
                    &META,
                    name.text_range(),
                    format!(
                        "field `{}` is explicitly initialized to `{}`, its default value",
                        name.text(),
                        literal.text()
                    ),
                )
                .with_fix("remove the initializer", vec![Edit::delete(initializer)]),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_zero;

    #[test]
    fn zeros() {
        for zero in [
            "0", "0L", "00", "0x0", "0x00L", "0.0", "0f", "0.0d", "0e5", "0_0", "0b0", ".0",
        ] {
            assert!(is_zero(zero), "{zero}");
        }
        for other in ["1", "0x10", "0.1", "10", "0xf", "1e0", "0b1", "08"] {
            assert!(!is_zero(other), "{other}");
        }
    }
}
