//! A Java formatter whose target is output identical to google-java-format.
//!
//! Formatting runs in three steps: parse the source into a lossless syntax
//! tree, turn the tree into a layout document that says where lines may
//! break ([`format`](mod@format) and [`spacing`]), and print the document
//! within the line width ([`printer`]).
//!
//! The result is then checked against the input: formatting may only change
//! whitespace and the order of imports. If any other token or comment would
//! be added, dropped or changed, [`format`] returns an error instead of the
//! text, so a formatter bug cannot silently damage code.

mod comments;
mod doc;
mod format;
mod javadoc;
mod printer;
mod spacing;
mod verify;

use weavejig_syntax::{SyntaxError, parse};

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
    let root = parse.syntax();
    let doc = format::Formatter::new(&root, options).node(&root);
    let formatted = printer::print(&doc, FormatOptions::LINE_WIDTH);
    verify::check(text, &formatted).map_err(FormatError::Internal)?;
    Ok(formatted)
}
