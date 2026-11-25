//! Sorts the files the formatter gets wrong by what kind of difference
//! there is between its output and google-java-format's.
//!
//! The formatter's score is one number per corpus. These kinds say where
//! the rest of it is, and they are totals, so they can be printed for the
//! held-out set too without showing any of its files.

use serde::Serialize;
use weavejig_syntax::{SyntaxKind, lex};

/// What separates two outputs for one file. A file is given the first kind
/// that accounts for the whole difference, in the order tried by
/// [`classify`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Only the whitespace inside text blocks.
    TextBlocks,
    /// Only Javadoc comments.
    Javadoc,
    /// Only Javadoc comments and the whitespace inside text blocks.
    JavadocAndTextBlocks,
    /// The tokens themselves, or the text of comments that are not Javadoc:
    /// string literals split differently, imports kept or removed, `//`
    /// comments wrapped differently.
    Tokens,
    /// Only empty lines, once Javadoc and text blocks are set aside.
    BlankLines,
    /// Where lines break and how they are indented.
    Wrapping,
}

impl Kind {
    pub const ALL: [Kind; 6] = [
        Kind::Wrapping,
        Kind::Javadoc,
        Kind::TextBlocks,
        Kind::JavadocAndTextBlocks,
        Kind::BlankLines,
        Kind::Tokens,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Kind::TextBlocks => "text blocks",
            Kind::Javadoc => "Javadoc",
            Kind::JavadocAndTextBlocks => "Javadoc and text blocks",
            Kind::Tokens => "tokens or comment text",
            Kind::BlankLines => "blank lines",
            Kind::Wrapping => "line wrapping",
        }
    }
}

/// The kind of difference between two outputs for one file, which must
/// differ.
pub fn classify(expected: &str, actual: &str) -> Kind {
    let same = |text_blocks, javadoc| {
        normalised(expected, text_blocks, javadoc) == normalised(actual, text_blocks, javadoc)
    };
    if same(true, false) {
        return Kind::TextBlocks;
    }
    if same(false, true) {
        return Kind::Javadoc;
    }
    let (expected, actual) = (
        normalised(expected, true, true),
        normalised(actual, true, true),
    );
    if expected == actual {
        return Kind::JavadocAndTextBlocks;
    }
    let squeezed = |text: &str| -> String { text.chars().filter(|c| !c.is_whitespace()).collect() };
    if squeezed(&expected) != squeezed(&actual) {
        return Kind::Tokens;
    }
    let lines = |text: &str| -> Vec<String> {
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .map(str::to_string)
            .collect()
    };
    if lines(&expected) == lines(&actual) {
        Kind::BlankLines
    } else {
        Kind::Wrapping
    }
}

/// The 1-based number of the first line on which two outputs differ.
pub fn first_difference(expected: &str, actual: &str) -> usize {
    let mut actual = actual.lines();
    for (i, line) in expected.lines().enumerate() {
        if actual.next() != Some(line) {
            return i + 1;
        }
    }
    expected.lines().count() + 1
}

/// `text` with the whitespace inside its text blocks removed, or its
/// Javadoc comments emptied, or both.
fn normalised(text: &str, text_blocks: bool, javadoc: bool) -> String {
    let (tokens, _) = lex(text);
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for token in tokens {
        let end = at + usize::from(token.len);
        let piece = &text[at..end];
        at = end;
        match token.kind {
            SyntaxKind::TEXT_BLOCK if text_blocks => {
                // A `\` that ends a line joins it to the next: it goes with
                // the whitespace, since moving the closing delimiter to a
                // line of its own adds one.
                let joined = piece.replace("\\\r\n", "").replace("\\\n", "");
                out.extend(joined.chars().filter(|c| !c.is_whitespace()));
            }
            SyntaxKind::BLOCK_COMMENT if javadoc && piece.starts_with("/**") => {
                out.push_str("/** */");
            }
            _ => out.push_str(piece),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        let expected = "/** A b. */\nclass A {\n  String s =\n      \"\"\"\n      x\n      \"\"\";\n\n  int f(int a) {\n    return a;\n  }\n}\n";
        let javadoc = expected.replace("/** A b. */", "/**\n * A b.\n */");
        assert_eq!(classify(expected, &javadoc), Kind::Javadoc);
        let text_block = expected.replace("      x\n      \"\"\"", "    x\n    \"\"\"");
        assert_eq!(classify(expected, &text_block), Kind::TextBlocks);
        let closing = expected.replace("      x\n      \"\"\"", "      x\\\n      \"\"\"");
        assert_eq!(classify(expected, &closing), Kind::TextBlocks);
        let both = javadoc.replace("      x\n      \"\"\"", "    x\n    \"\"\"");
        assert_eq!(classify(expected, &both), Kind::JavadocAndTextBlocks);
        let blank = expected.replace(";\n\n  int", ";\n  int");
        assert_eq!(classify(expected, &blank), Kind::BlankLines);
        let wrapping = expected.replace("int f(int a) {", "int f(\n      int a) {");
        assert_eq!(classify(expected, &wrapping), Kind::Wrapping);
        let tokens = expected.replace("return a;", "return (a);");
        assert_eq!(classify(expected, &tokens), Kind::Tokens);
        // A `"""` inside a comment is not a text block.
        let comment = "// \"\"\"\nclass A {}\n";
        assert_eq!(
            classify(comment, "// \"\"\"\nclass A {\n}\n"),
            Kind::Wrapping
        );
    }

    #[test]
    fn first_differing_line() {
        assert_eq!(first_difference("a\nb\nc\n", "a\nx\nc\n"), 2);
        assert_eq!(first_difference("a\nb\n", "a\nb\nc\n"), 3);
        assert_eq!(first_difference("a\nb\nc\n", "a\nb\n"), 3);
    }
}
