use weavejig_syntax::{SyntaxKind::*, SyntaxNode, SyntaxToken};

use crate::ast;
use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// A number in the middle of code says nothing about what it means, and has
/// to be found again wherever it is repeated. Give it a name: a constant.
///
/// A number is not magic where it defines a constant (a `final` variable,
/// an interface field or an enum constant), as long as nothing stands
/// between it and the constant but arithmetic, comparisons, conditionals,
/// casts, array initializers and calls: `static final long DAY = 24 * HOUR;`
/// is fine.
pub(crate) struct MagicNumber;

static META: RuleMeta = RuleMeta::new(
    "magic-number",
    "numbers other than -1, 0, 1 and 2 should be named constants",
)
.checkstyle("MagicNumber")
.opt_in()
.options(&[
    OptionSpec {
        name: "ignore-numbers",
        default: OptionDefault::List(&["-1", "0", "1", "2"]),
        help: "numbers that are never magic",
    },
    OptionSpec {
        name: "ignore-hash-code-method",
        default: OptionDefault::Bool(false),
        help: "allow any number in a `hashCode` method",
    },
    OptionSpec {
        name: "ignore-annotation",
        default: OptionDefault::Bool(false),
        help: "allow any number in an annotation",
    },
    OptionSpec {
        name: "ignore-field-declaration",
        default: OptionDefault::Bool(false),
        help: "allow any number in the initializer of a field",
    },
    OptionSpec {
        name: "ignore-annotation-element-defaults",
        default: OptionDefault::Bool(true),
        help: "allow any number as the default value of an annotation element",
    },
]);

/// The value of a numeric literal, as far as telling numbers apart needs.
fn value(literal: &str) -> Option<f64> {
    let text = literal.to_ascii_lowercase().replace('_', "");
    let radix = if text.starts_with("0x") {
        16
    } else if text.starts_with("0b") {
        2
    } else {
        10
    };
    if radix == 16 && text.contains(['.', 'p']) {
        // A hexadecimal floating-point literal; nobody ignores those.
        return None;
    }
    let is_long = text.ends_with('l');
    let digits = text.trim_end_matches('l');
    let integer = |digits: &str, radix: u32| {
        u64::from_str_radix(digits, radix).ok().map(|value| {
            // A literal with its top bit set is negative: `0xFFFFFFFF` is -1.
            if !is_long && value > i32::MAX as u64 {
                f64::from(value as u32 as i32)
            } else if is_long && value > i64::MAX as u64 {
                value as i64 as f64
            } else {
                value as f64
            }
        })
    };
    if radix != 10 {
        return integer(&digits[2..], radix);
    }
    let is_float = !is_long && text.contains(['.', 'e', 'f', 'd']);
    if is_float {
        return text.trim_end_matches(['f', 'd']).parse().ok();
    }
    if digits.len() > 1 && digits.starts_with('0') {
        return integer(&digits[1..], 8);
    }
    digits.parse().ok()
}

/// The literal's value with the sign of a `-` in front of it, and the
/// outermost node of the signed number.
fn signed(literal: &SyntaxNode, token: &SyntaxToken) -> (Option<f64>, SyntaxNode) {
    let magnitude = value(token.text());
    match literal.parent() {
        Some(parent) if parent.kind() == PREFIX_EXPR && ast::operator(&parent) == "-" => {
            (magnitude.map(|value| -value), parent)
        }
        _ => (magnitude, literal.clone()),
    }
}

/// Whether everything between `number` and the constant `definition` it is
/// part of is something a constant's value may be built with.
fn only_builds_value(number: &SyntaxNode, definition: &SyntaxNode) -> bool {
    let mut child = number.clone();
    while let Some(parent) = child.parent() {
        if parent == *definition {
            return true;
        }
        let allowed = match parent.kind() {
            PAREN_EXPR | CAST_EXPR | ARG_LIST | ARRAY_INIT | VARIABLE_DECLARATOR | NEW_EXPR
            | ARRAY_CREATION_EXPR | ENUM_CONSTANT | CONDITIONAL_EXPR => true,
            // The number may be an argument, but not what a member is
            // selected from: `Duration.ofDays(7)` but not `(7).x`.
            METHOD_CALL => child.kind() == ARG_LIST,
            PREFIX_EXPR => matches!(ast::operator(&parent).as_str(), "-" | "+" | "~"),
            // Arithmetic and comparisons, but not `&&` and `||`.
            BINARY_EXPR => !matches!(ast::operator(&parent).as_str(), "&&" | "||"),
            _ => false,
        };
        if !allowed {
            return false;
        }
        child = parent;
    }
    false
}

impl Rule for MagicNumber {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let Some(token) =
            ast::literal(node).filter(|token| matches!(token.kind(), INT_LITERAL | FLOAT_LITERAL))
        else {
            return;
        };
        let (number, outer) = signed(node, &token);
        let ignored = ctx.options.list("ignore-numbers").iter().any(|ignored| {
            let ignored = ignored.trim();
            let (negative, magnitude) = match ignored.strip_prefix('-') {
                Some(magnitude) => (true, magnitude),
                None => (false, ignored),
            };
            let ignored = value(magnitude).map(|value| if negative { -value } else { value });
            // By bits: `-0.0` is not the `0` that is ignored.
            number.is_some() && number.map(f64::to_bits) == ignored.map(f64::to_bits)
        });
        if ignored {
            return;
        }

        // What the number is part of: a variable or enum constant that may
        // make it a named constant, and the places where any number goes.
        let mut definition = None;
        for ancestor in node.ancestors().skip(1) {
            match ancestor.kind() {
                ANNOTATION if ctx.options.bool("ignore-annotation") => return,
                DEFAULT_VALUE if ctx.options.bool("ignore-annotation-element-defaults") => return,
                METHOD_DECL
                    if ctx.options.bool("ignore-hash-code-method")
                        && ast::name_text(&ancestor) == "hashCode"
                        && ast::params(&ancestor).is_empty() =>
                {
                    return;
                }
                FIELD_DECL if ctx.options.bool("ignore-field-declaration") => return,
                FIELD_DECL | LOCAL_VAR_DECL | ENUM_CONSTANT if definition.is_none() => {
                    definition = Some(ancestor);
                }
                // A number in a method or lambda is not part of the value
                // of a variable declared around it.
                METHOD_DECL | CONSTRUCTOR_DECL | LAMBDA_EXPR | CLASS_BODY | INITIALIZER_BLOCK
                    if definition.is_none() =>
                {
                    break;
                }
                _ => {}
            }
        }
        if let Some(definition) = definition {
            let is_constant = definition.kind() == ENUM_CONSTANT
                || ast::has_modifier(&definition, FINAL_KW)
                || (definition.kind() == FIELD_DECL
                    && ast::owner(&definition).is_some_and(|owner| ast::is_interface(&owner)));
            if is_constant && only_builds_value(&outer, &definition) {
                return;
            }
        }
        out.push(Diagnostic::new(
            &META,
            outer.text_range(),
            format!("`{}` is a magic number", ast::compact(&outer)),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::value;

    #[test]
    fn values() {
        assert_eq!(value("0"), Some(0.0));
        assert_eq!(value("1L"), Some(1.0));
        assert_eq!(value("2.0f"), Some(2.0));
        assert_eq!(value("0x10"), Some(16.0));
        assert_eq!(value("010"), Some(8.0));
        assert_eq!(value("0b101"), Some(5.0));
        assert_eq!(value("1_000"), Some(1000.0));
        assert_eq!(value("1e3"), Some(1000.0));
        assert_eq!(value("0xFFFFFFFF"), Some(-1.0));
        assert_eq!(value("0xFFFFFFFFFFFFFFFFL"), Some(-1.0));
        assert_eq!(value(".5"), Some(0.5));
        assert_eq!(value("0L"), Some(0.0));
        assert_eq!(value("00"), Some(0.0));
        assert_eq!(value("1d"), Some(1.0));
        assert_eq!(value("0x1p3"), None);
    }
}
