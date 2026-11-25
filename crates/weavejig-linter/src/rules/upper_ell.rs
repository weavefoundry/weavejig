use weavejig_syntax::{SyntaxKind, SyntaxToken, TextRange, TextSize};

use crate::{Context, Diagnostic, Edit, Fix, Rule, RuleMeta};

/// Long literals must end in an upper-case `L`: a lower-case `l` is easily
/// mistaken for the digit `1`.
pub(crate) struct UpperEll;

static META: RuleMeta = RuleMeta::new(
    "upper-ell",
    "long literals should use an upper-case `L` suffix",
)
.checkstyle("UpperEll")
.fixable();

impl Rule for UpperEll {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn checks_tokens(&self) -> bool {
        true
    }

    fn check_token(&self, token: &SyntaxToken, _ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if token.kind() != SyntaxKind::INT_LITERAL || !token.text().ends_with('l') {
            return;
        }
        let range = token.text_range();
        let suffix = TextRange::new(range.end() - TextSize::new(1), range.end());
        out.push(Diagnostic {
            rule: META.id,
            message: "long literal should use an upper-case `L` suffix".to_string(),
            range,
            fix: Some(Fix {
                description: "use an upper-case `L`".to_string(),
                edits: vec![Edit::replace(suffix, "L")],
            }),
        });
    }
}
