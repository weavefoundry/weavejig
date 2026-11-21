//! Decides which token each comment belongs to.
//!
//! Comments are not part of the grammar, so the formatter carries them along
//! with tokens: a comment that ends the line a token is on trails that token,
//! and any other comment leads the next token.

use std::collections::HashMap;

use weavejig_syntax::{SyntaxKind, SyntaxNode, TextSize};

#[derive(Debug, Clone)]
pub(crate) struct Comment {
    pub(crate) text: String,
    /// `// ...` rather than `/* ... */`.
    pub(crate) is_line: bool,
    /// Line breaks between the previous token or comment and this comment.
    pub(crate) newlines_before: u32,
}

impl Comment {
    pub(crate) fn is_javadoc(&self) -> bool {
        self.text.starts_with("/**") && self.text.len() > 4
    }
}

#[derive(Debug, Default)]
pub(crate) struct TokenTrivia {
    /// Comments printed before the token.
    pub(crate) leading: Vec<Comment>,
    /// Comments printed after the token, on its line.
    pub(crate) trailing: Vec<Comment>,
    /// Line breaks between the last leading comment (or, without one, the
    /// previous token) and the token.
    pub(crate) newlines_before: u32,
}

impl TokenTrivia {
    /// Whether the source has an empty line before the token, or before the
    /// first of its leading comments.
    pub(crate) fn blank_line_before(&self) -> bool {
        self.leading
            .first()
            .map_or(self.newlines_before, |c| c.newlines_before)
            >= 2
    }
}

#[derive(Debug, Default)]
pub(crate) struct Comments {
    /// Keyed by the start offset of the token.
    tokens: HashMap<TextSize, TokenTrivia>,
    /// Comments after the last token of the file.
    pub(crate) at_end: Vec<Comment>,
}

impl Comments {
    pub(crate) fn of(&self, token_start: TextSize) -> Option<&TokenTrivia> {
        self.tokens.get(&token_start)
    }

    pub(crate) fn collect(root: &SyntaxNode) -> Comments {
        let mut comments = Comments::default();
        let mut previous: Option<TextSize> = None;
        let mut pending: Vec<Comment> = Vec::new();
        let mut newlines = 0u32;

        let mut token = root.first_token();
        while let Some(current) = token {
            match current.kind() {
                SyntaxKind::WHITESPACE => newlines += count_newlines(current.text()),
                kind if kind.is_comment() => {
                    pending.push(Comment {
                        text: current.text().to_string(),
                        is_line: kind == SyntaxKind::LINE_COMMENT,
                        newlines_before: newlines,
                    });
                    newlines = 0;
                }
                _ => {
                    let start = current.text_range().start();
                    let trailing = split_trailing(&mut pending, newlines, previous.is_some());
                    if let (Some(previous), false) = (previous, trailing.is_empty()) {
                        comments.tokens.entry(previous).or_default().trailing = trailing;
                    }
                    if !pending.is_empty() || newlines > 0 {
                        let trivia = comments.tokens.entry(start).or_default();
                        trivia.leading = std::mem::take(&mut pending);
                        trivia.newlines_before = newlines;
                    }
                    previous = Some(start);
                    newlines = 0;
                }
            }
            token = current.next_token();
        }

        // End of input ends the line, so same-line comments trail the last token.
        let trailing = split_trailing(&mut pending, 1, previous.is_some());
        if let (Some(previous), false) = (previous, trailing.is_empty()) {
            comments.tokens.entry(previous).or_default().trailing = trailing;
        }
        comments.at_end = pending;
        comments
    }
}

/// Removes and returns the comments at the front of `pending` that trail the
/// previous token: those on its line, provided the line ends after them.
/// `newlines_after` is the number of line breaks after the last pending
/// comment.
fn split_trailing(
    pending: &mut Vec<Comment>,
    newlines_after: u32,
    has_previous: bool,
) -> Vec<Comment> {
    if !has_previous {
        return Vec::new();
    }
    let same_line = pending
        .iter()
        .take_while(|c| c.newlines_before == 0)
        .count();
    let line_ends = match pending.get(same_line) {
        Some(next) => next.newlines_before > 0,
        None => newlines_after > 0,
    };
    if same_line == 0 || !line_ends {
        return Vec::new();
    }
    pending.drain(..same_line).collect()
}

fn count_newlines(text: &str) -> u32 {
    let bytes = text.as_bytes();
    let mut count = 0;
    for (i, byte) in bytes.iter().enumerate() {
        match byte {
            b'\n' => count += 1,
            b'\r' if bytes.get(i + 1) != Some(&b'\n') => count += 1,
            _ => {}
        }
    }
    count
}
