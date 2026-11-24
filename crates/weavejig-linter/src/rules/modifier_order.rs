use weavejig_syntax::{
    SyntaxKind::*, SyntaxNode, TextRange, child_token, modifier_rank as rank, significant_children,
};

use crate::ast;

use crate::{Context, Diagnostic, Edit, Fix, Rule, RuleMeta};

/// Modifiers should appear in the order the Java Language Specification
/// suggests, with annotations before all keywords.
pub(crate) struct ModifierOrder;

static META: RuleMeta = RuleMeta::new(
    "modifier-order",
    "modifiers should follow the order suggested by the JLS",
)
.checkstyle("ModifierOrder")
.fixable();

struct Modifier {
    /// The keyword, or `None` for an annotation.
    keyword: Option<String>,
    range: TextRange,
}

fn modifiers(node: &SyntaxNode) -> Vec<Modifier> {
    let mut out: Vec<Modifier> = Vec::new();
    for element in significant_children(node) {
        let range = element.text_range();
        match element.as_token() {
            None => out.push(Modifier {
                keyword: None,
                range,
            }),
            // `non-sealed` is three tokens: `non`, `-`, `sealed`.
            Some(token) if token.kind() == MINUS => {}
            Some(token) => match out.last_mut() {
                Some(Modifier {
                    keyword: Some(last),
                    range: last_range,
                }) if last == "non" && token.text() == "sealed" => {
                    *last = "non-sealed".to_string();
                    *last_range = last_range.cover(range);
                }
                _ => out.push(Modifier {
                    keyword: Some(token.text().to_string()),
                    range,
                }),
            },
        }
    }
    out
}

impl Rule for ModifierOrder {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if node.kind() != MODIFIERS {
            return;
        }
        let all = modifiers(node);
        // Leading annotations are in the right place.
        let leading = all.iter().take_while(|m| m.keyword.is_none()).count();
        let rest = &all[leading..];

        // An annotation after a keyword is out of place, unless it may be a
        // type annotation: on a variable, parameter or constructor, or on a
        // method that returns something (`public @Nullable String f()`).
        let may_annotate_type = match node.parent() {
            Some(owner) => match owner.kind() {
                METHOD_DECL => {
                    ast::declared_type(&owner).is_some_and(|ty| child_token(&ty, VOID_KW).is_none())
                }
                kind => !kind.is_type_decl(),
            },
            None => true,
        };
        let misplaced = (!may_annotate_type)
            .then(|| rest.iter().position(|m| m.keyword.is_none()))
            .flatten();

        let text = |m: &Modifier| m.keyword.clone().unwrap_or_default();
        let keywords: Vec<(usize, &Modifier)> = rest
            .iter()
            .enumerate()
            .filter(|(_, m)| m.keyword.is_some())
            .collect();
        let offender = keywords
            .windows(2)
            .find(|pair| rank(&text(pair[0].1)) > rank(&text(pair[1].1)))
            .map(|pair| pair[1]);

        // Whichever comes first is reported.
        if let Some(annotation) = misplaced
            && offender.is_none_or(|(at, _)| annotation < at)
        {
            out.push(Diagnostic {
                rule: META.id,
                message: "annotation should come before all modifier keywords".to_string(),
                range: rest[annotation].range,
                fix: None,
            });
            return;
        }
        let Some((_, offender)) = offender else {
            return;
        };
        let keywords: Vec<&Modifier> = keywords.into_iter().map(|(_, m)| m).collect();

        let mut sorted: Vec<String> = keywords.iter().map(|m| text(m)).collect();
        sorted.sort_by_key(|keyword| rank(keyword));
        let edits = keywords
            .iter()
            .zip(&sorted)
            .filter(|(modifier, keyword)| text(modifier) != **keyword)
            .map(|(modifier, keyword)| Edit::replace(modifier.range, keyword.clone()))
            .collect();
        out.push(Diagnostic {
            rule: META.id,
            message: format!(
                "`{}` is out of order; the JLS suggests `{}`",
                text(offender),
                sorted.join(" ")
            ),
            range: offender.range,
            fix: Some(Fix {
                description: "reorder the modifiers".to_string(),
                edits,
            }),
        });
    }
}
