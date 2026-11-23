//! Modifier order.
//!
//! google-java-format puts modifier keywords in the order the JLS suggests
//! ([`MODIFIER_ORDER`](weavejig_syntax::MODIFIER_ORDER)), before it lays the
//! file out. [`reorder`] does the same to the source text. The rules, as
//! observed:
//!
//! - Only keywords move. An annotation between two keywords splits them into
//!   runs that are sorted separately (`static @A final` stays as it is), and
//!   annotations never move.
//! - Comments stay where they are; the keywords around them trade places
//!   (`final /* c */ static` becomes `static /* c */ final`).

use weavejig_syntax::{
    NodeOrToken, SyntaxKind::*, SyntaxNode, TextRange, modifier_rank, significant_children,
};

use crate::Edit;

/// A modifier keyword. `non-sealed` is three tokens, `non`, `-` and
/// `sealed`, and one keyword.
pub(crate) struct Keyword {
    pub(crate) text: String,
    pub(crate) range: TextRange,
}

/// The runs of keywords in every modifier list of the tree: consecutive
/// keywords with no annotation between them.
pub(crate) fn runs(root: &SyntaxNode) -> Vec<Vec<Keyword>> {
    let mut runs = Vec::new();
    for node in root.descendants().filter(|node| node.kind() == MODIFIERS) {
        let mut run: Vec<Keyword> = Vec::new();
        for element in significant_children(&node) {
            match element {
                NodeOrToken::Node(_) => runs.extend(Some(std::mem::take(&mut run))),
                NodeOrToken::Token(token) => {
                    let range = token.text_range();
                    match run.last_mut() {
                        Some(last) if last.text == "non" && token.kind() == MINUS => {
                            last.text.push('-');
                            last.range = last.range.cover(range);
                        }
                        Some(last) if last.text == "non-" && token.text() == "sealed" => {
                            last.text.push_str("sealed");
                            last.range = last.range.cover(range);
                        }
                        _ => run.push(Keyword {
                            text: token.text().to_string(),
                            range,
                        }),
                    }
                }
            }
        }
        runs.push(run);
    }
    runs.retain(|run| !run.is_empty());
    runs
}

/// The edits that put every run of modifier keywords in order.
pub(crate) fn reorder(root: &SyntaxNode) -> Vec<Edit> {
    let mut edits = Vec::new();
    for run in runs(root) {
        let mut sorted: Vec<&str> = run.iter().map(|keyword| keyword.text.as_str()).collect();
        sorted.sort_by_key(|keyword| modifier_rank(keyword));
        for (keyword, text) in run.iter().zip(sorted) {
            if keyword.text != text {
                edits.push(Edit {
                    range: keyword.range,
                    text: text.to_string(),
                });
            }
        }
    }
    edits
}
