//! A Java formatter whose target is output identical to google-java-format.
//!
//! Formatting runs in three steps: parse the source into a lossless syntax
//! tree, turn the tree into a layout document that says where lines may
//! break ([`format`](mod@format) and [`spacing`]), and print the document
//! within the line width ([`printer`]).
//!
//! Before that, the source text itself is fixed up as google-java-format
//! does: modifier keywords are put in order ([`modifiers`]), unused imports
//! are removed and the rest sorted ([`imports`]), and tokens that do
//! nothing, such as the `;` of an empty statement, are dropped
//! ([`redundant`]).
//!
//! Finally, string literals that run past the line width are split into
//! concatenations of shorter ones ([`strings`]).
//!
//! The result is then checked against the input: formatting may only change
//! whitespace, the order of imports and of modifier keywords, which imports
//! there are (only unused ones may go), how string literals joined by `+`
//! are split, the indentation of text blocks ([`textblock`]), which is not
//! part of their value, and whether the tokens that do nothing are there. If any other token or comment would be added, dropped or
//! changed, [`format`] returns an error instead of the text, so a formatter
//! bug cannot silently damage code.

mod comments;
mod doc;
mod format;
mod imports;
mod javadoc;
mod modifiers;
mod printer;
mod redundant;
mod spacing;
mod strings;
mod textblock;
mod verify;

use weavejig_syntax::{SyntaxError, SyntaxNode, TextRange, TextSize, parse};

/// The two styles google-java-format offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Style {
    /// Google Java Style: 2-space blocks, 4-space continuations.
    #[default]
    Google,
    /// Android Open Source Project style: 4-space blocks, 8-space continuations.
    Aosp,
}

impl Style {
    pub(crate) fn block_indent(self) -> u16 {
        match self {
            Style::Google => 2,
            Style::Aosp => 4,
        }
    }

    pub(crate) fn continuation_indent(self) -> u16 {
        self.block_indent() * 2
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FormatOptions {
    pub style: Style,
}

impl FormatOptions {
    /// Both styles wrap at 100 columns.
    pub const LINE_WIDTH: usize = 100;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// The source is not valid Java, so it is left alone.
    Syntax(Vec<SyntaxError>),
    /// The formatter would have changed more than whitespace. This is a bug
    /// in WeaveJig; the message says what differed.
    Internal(String),
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatError::Syntax(errors) => {
                let first = errors
                    .first()
                    .map_or("syntax error", |e| e.message.as_str());
                write!(f, "cannot format invalid Java: {first}")
            }
            FormatError::Internal(what) => {
                write!(
                    f,
                    "internal formatter error ({what}); the file was left unchanged"
                )
            }
        }
    }
}

impl std::error::Error for FormatError {}

/// Formats a Java compilation unit.
pub fn format(text: &str, options: &FormatOptions) -> Result<String, FormatError> {
    let parse = parse(text);
    if !parse.is_ok() {
        return Err(FormatError::Syntax(parse.errors().to_vec()));
    }
    let fixed = fix(text, &parse.syntax());
    let parse = match &fixed {
        Some(fixed) => weavejig_syntax::parse(fixed),
        None => parse,
    };
    if !parse.is_ok() {
        return Err(FormatError::Internal(
            "reordering modifiers or imports broke the syntax".to_string(),
        ));
    }
    let root = parse.syntax();
    let doc = doc::place_comment_breaks(format::Formatter::new(&root, options).node(&root));
    let formatted = printer::print(&doc, FormatOptions::LINE_WIDTH);
    let formatted = strings::reflow(&formatted, FormatOptions::LINE_WIDTH).unwrap_or(formatted);
    let formatted = with_line_separator(&formatted, line_separator(text));
    verify::check(text, &formatted).map_err(FormatError::Internal)?;
    Ok(formatted)
}

/// A replacement of part of the source text.
pub(crate) struct Edit {
    pub(crate) range: TextRange,
    pub(crate) text: String,
}

/// The source with modifiers reordered, unused imports removed and tokens
/// that do nothing dropped, or `None` if none of that changes anything.
fn fix(text: &str, root: &SyntaxNode) -> Option<String> {
    let mut edits = modifiers::reorder(root);
    edits.extend(imports::fix(text, root));
    edits.extend(redundant::tokens(root).into_iter().map(|range| Edit {
        range: whole_line(text, range),
        text: String::new(),
    }));
    if edits.is_empty() {
        return None;
    }
    edits.sort_by_key(|edit| edit.range.start());
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for edit in edits {
        let start = usize::from(edit.range.start());
        out.push_str(&text[at..start]);
        out.push_str(&edit.text);
        at = usize::from(edit.range.end());
    }
    out.push_str(&text[at..]);
    Some(out)
}

/// `range`, widened to its whole line with the line break if nothing else is
/// on the line: removing a token must not leave an empty line behind.
fn whole_line(text: &str, range: TextRange) -> TextRange {
    let (start, end) = (usize::from(range.start()), usize::from(range.end()));
    let blank = |part: &str| part.chars().all(|c| c == ' ' || c == '\t');
    let line_start = text[..start].rfind('\n').map_or(0, |newline| newline + 1);
    let Some(line_end) = text[end..].find('\n').map(|newline| end + newline + 1) else {
        return range;
    };
    let rest = text[end..line_end].trim_end_matches(['\n', '\r']);
    if !blank(&text[line_start..start]) || !blank(rest) {
        return range;
    }
    match (TextSize::try_from(line_start), TextSize::try_from(line_end)) {
        (Ok(from), Ok(to)) => TextRange::new(from, to),
        _ => range,
    }
}

/// The line separator to write, which is the first one in `text`, as
/// google-java-format chooses it.
fn line_separator(text: &str) -> &'static str {
    match text.find(['\r', '\n']) {
        Some(i) if text[i..].starts_with("\r\n") => "\r\n",
        Some(i) if text.as_bytes()[i] == b'\r' => "\r",
        _ => "\n",
    }
}

/// `text` with every line terminator replaced by `separator`. This covers
/// the printer's own line breaks and those inside text blocks and comments,
/// so a file never ends up with mixed line endings.
fn with_line_separator(text: &str, separator: &str) -> String {
    if separator == "\n" && !text.contains('\r') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + text.len() / 32);
    let mut rest = text;
    while let Some(i) = rest.find(['\r', '\n']) {
        out.push_str(&rest[..i]);
        out.push_str(separator);
        let len = if rest[i..].starts_with("\r\n") { 2 } else { 1 };
        rest = &rest[i + len..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_endings_follow_the_input() {
        let options = FormatOptions::default();
        assert_eq!(
            format("class A{int x;}\r\n", &options).unwrap(),
            "class A {\r\n  int x;\r\n}\r\n"
        );
        assert_eq!(
            format("class A{int x;}\r", &options).unwrap(),
            "class A {\r  int x;\r}\r"
        );
        assert_eq!(
            format("class A{int x;}\n", &options).unwrap(),
            "class A {\n  int x;\n}\n"
        );
        // Text blocks and block comments follow the file, too: a CRLF file
        // formats exactly as its LF twin does, with CRLF everywhere, even
        // when a text block inside it used LF.
        let lf = "class A {\n  /* a\n     b */\n  String s = \"\"\"\n      x\n      \"\"\";\n}\n";
        let expected = format(lf, &options).unwrap().replace('\n', "\r\n");
        assert_eq!(
            format(&lf.replace('\n', "\r\n"), &options).unwrap(),
            expected
        );
        let mixed = lf
            .replace('\n', "\r\n")
            .replace("\"\"\"\r\n      x\r\n", "\"\"\"\n      x\n");
        assert_eq!(format(&mixed, &options).unwrap(), expected);
    }
}
