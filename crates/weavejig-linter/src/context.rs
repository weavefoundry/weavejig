//! What a rule knows about the file it is checking.

use std::path::Path;

use weavejig_syntax::{LineIndex, SyntaxNode, SyntaxToken, TextRange, TextSize};

use crate::Options;

/// A source file to lint: its text, and its path if it has one.
#[derive(Debug, Clone, Copy)]
pub struct Source<'a> {
    pub text: &'a str,
    /// Where the file is, for rules about file names. `None` for text that
    /// did not come from a file, such as standard input.
    pub path: Option<&'a Path>,
}

impl<'a> Source<'a> {
    pub fn new(text: &'a str, path: Option<&'a Path>) -> Source<'a> {
        Source { text, path }
    }
}

impl<'a> From<&'a str> for Source<'a> {
    fn from(text: &'a str) -> Source<'a> {
        Source { text, path: None }
    }
}

impl<'a> From<&'a String> for Source<'a> {
    fn from(text: &'a String) -> Source<'a> {
        Source { text, path: None }
    }
}

/// The file being checked, and the options of the rule checking it.
pub struct Context<'a> {
    pub text: &'a str,
    pub path: Option<&'a Path>,
    pub root: &'a SyntaxNode,
    pub options: &'a Options,
    lines: &'a LineIndex,
}

impl<'a> Context<'a> {
    pub(crate) fn new(
        source: Source<'a>,
        root: &'a SyntaxNode,
        lines: &'a LineIndex,
        options: &'a Options,
    ) -> Context<'a> {
        Context {
            text: source.text,
            path: source.path,
            root,
            options,
            lines,
        }
    }

    /// The 1-based line `offset` is on.
    pub fn line(&self, offset: TextSize) -> u32 {
        self.lines.line_col(self.text, offset).line
    }

    /// The 1-based line a token starts on.
    pub fn line_of(&self, token: &SyntaxToken) -> u32 {
        self.line(token.text_range().start())
    }

    /// The 1-based line a node's first token starts on.
    pub fn first_line(&self, node: &SyntaxNode) -> u32 {
        match weavejig_syntax::first_token(node) {
            Some(token) => self.line_of(&token),
            None => self.line(node.text_range().start()),
        }
    }

    /// The 1-based line a node's last token ends on.
    pub fn last_line(&self, node: &SyntaxNode) -> u32 {
        match weavejig_syntax::last_token(node) {
            Some(token) => self.line(token.text_range().end()),
            None => self.line(node.text_range().end()),
        }
    }

    /// The 1-based column, in characters, of `offset`.
    pub fn column(&self, offset: TextSize) -> u32 {
        self.lines.line_col(self.text, offset).column
    }

    /// How many lines the file has. A final line terminator does not start
    /// another line.
    pub fn line_count(&self) -> u32 {
        let count = self.lines.line_count() as u32;
        if self.text.is_empty() || self.text.ends_with(['\n', '\r']) {
            count - 1
        } else {
            count
        }
    }

    /// The range of a 1-based line, without its terminator.
    pub fn line_range(&self, line: u32) -> TextRange {
        self.lines.line_range(self.text, line)
    }

    /// The text of a 1-based line, without its terminator.
    pub fn line_text(&self, line: u32) -> &'a str {
        &self.text[self.line_range(line)]
    }
}
