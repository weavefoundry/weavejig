use weavejig_syntax::{
    SyntaxKind::*, SyntaxNode, TextRange, modifier_rank as rank, significant_children,
};

use crate::{Diagnostic, Edit, Fix, Rule, RuleMeta};

/// Modifiers should appear in the order the Java Language Specification
/// suggests, with annotations before all keywords.
pub(crate) struct ModifierOrder;

static META: RuleMeta = RuleMeta {
    id: "modifier-order",
    summary: "modifiers should follow the order suggested by the JLS",
    checkstyle: Some("ModifierOrder"),
    pmd: None,
    fixable: true,
};

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

    fn check_node(&self, node: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        if node.kind() != MODIFIERS {
            return;
        }
        let all = modifiers(node);
        // Leading annotations are in the right place.
        let leading = all.iter().take_while(|m| m.keyword.is_none()).count();
        let rest = &all[leading..];

        // An annotation between keywords is out of place. Those after the
        // last keyword are type annotations (`public @Nullable String f()`,
        // `private final @NonNull @Valid Foo x`).
        let last_keyword = rest.iter().rposition(|m| m.keyword.is_some());
        if let Some(annotation) = rest
            .iter()
            .enumerate()
            .find(|(i, m)| m.keyword.is_none() && last_keyword.is_some_and(|last| *i < last))
        {
            out.push(Diagnostic {
                rule: META.id,
                message: "annotation should come before all modifier keywords".to_string(),
                range: annotation.1.range,
                fix: None,
            });
            return;
        }

        let keywords: Vec<&Modifier> = rest.iter().filter(|m| m.keyword.is_some()).collect();
        let text = |m: &Modifier| m.keyword.clone().unwrap_or_default();
        let Some(offender) = keywords
            .windows(2)
            .find(|pair| rank(&text(pair[0])) > rank(&text(pair[1])))
        else {
            return;
        };

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
                text(offender[1]),
                sorted.join(" ")
            ),
            range: offender[1].range,
            fix: Some(Fix {
                description: "reorder the modifiers".to_string(),
                edits,
            }),
        });
    }
}
