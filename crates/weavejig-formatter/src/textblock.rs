//! Text blocks, re-indented as google-java-format does.
//!
//! The compiler strips from a text block the indentation its lines share
//! (JLS §3.10.6), so the block can move left or right without its value
//! changing. google-java-format lines the content and the closing
//! delimiter up with the line the opening delimiter is on, and gives the
//! closing delimiter a line of its own. A block whose lines share no
//! indentation at all is left at the margin, opening delimiter included.

/// A text block's lines, without the indentation they share.
pub(crate) struct TextBlock {
    /// The content lines. An empty line is empty.
    lines: Vec<String>,
    /// Whether the content ends without a line break: the closing delimiter
    /// was written on the last line of the content.
    open_ended: bool,
    /// Whether the lines share no indentation.
    pub(crate) at_margin: bool,
}

/// Whether Java counts `c` as white space when it strips indentation.
fn is_white(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\x0C')
}

impl TextBlock {
    /// Reads a text block token. `None` if it is not laid out as one, which
    /// leaves it as it is.
    pub(crate) fn parse(token: &str) -> Option<TextBlock> {
        let body = token.strip_prefix("\"\"\"")?.strip_suffix("\"\"\"")?;
        let body = body.replace("\r\n", "\n").replace('\r', "\n");
        let (first, content) = body.split_once('\n')?;
        if !first.chars().all(is_white) {
            return None;
        }
        let mut lines: Vec<&str> = content.split('\n').collect();
        let last = lines.pop()?;
        let open_ended = !last.chars().all(is_white);
        let blank = |line: &str| line.chars().all(is_white);
        let indent = |line: &str| line.chars().take_while(|c| is_white(*c)).count();
        // The line of the closing delimiter counts even if it is blank.
        let shared = lines
            .iter()
            .filter(|line| !blank(line))
            .map(|line| indent(line))
            .chain(std::iter::once(indent(last)))
            .min()
            .unwrap_or(0);
        let strip = |line: &str| -> String {
            if blank(line) {
                return String::new();
            }
            let start = line
                .char_indices()
                .nth(shared)
                .map_or(line.len(), |(index, _)| index);
            line[start..].trim_end_matches(is_white).to_string()
        };
        let mut stripped: Vec<String> = lines.iter().map(|line| strip(line)).collect();
        if open_ended {
            stripped.push(strip(last));
        }
        Some(TextBlock {
            lines: stripped,
            open_ended,
            at_margin: shared == 0,
        })
    }

    /// The block as it is printed with its lines indented by `indent`
    /// columns. The opening delimiter is not indented: it goes wherever
    /// the caller is on the line.
    pub(crate) fn print(&self, indent: usize) -> String {
        let pad = " ".repeat(indent);
        let mut out = String::from("\"\"\"\n");
        for (i, line) in self.lines.iter().enumerate() {
            if !line.is_empty() {
                out.push_str(&pad);
                out.push_str(line);
            }
            // The closing delimiter moves to a line of its own; a `\` keeps
            // that line break out of the string.
            if self.open_ended && i + 1 == self.lines.len() {
                out.push('\\');
            }
            out.push('\n');
        }
        out.push_str(&pad);
        out.push_str("\"\"\"");
        out
    }

    /// The block's content as the compiler reads it, short of interpreting
    /// escape sequences: what formatting must not change.
    fn value(&self) -> String {
        let mut value = self.lines.join("\n");
        if !self.open_ended {
            value.push('\n');
        }
        // A `\` that ends a line joins it to the next, unless the `\` is
        // itself escaped.
        let mut out = String::with_capacity(value.len());
        let mut backslashes = 0;
        for c in value.chars() {
            if c == '\n' && backslashes % 2 == 1 {
                out.pop();
            } else {
                out.push(c);
            }
            backslashes = if c == '\\' { backslashes + 1 } else { 0 };
        }
        out
    }
}

/// What a text block token says, whatever its indentation: equal for two
/// tokens exactly when the compiler reads the same string from both.
pub(crate) fn fingerprint(token: &str) -> String {
    match TextBlock::parse(token) {
        Some(block) => format!("\"\"\"{}\"\"\"", block.value()),
        None => token.replace("\r\n", "\n").replace('\r', "\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reprint(token: &str, indent: usize) -> String {
        TextBlock::parse(token).unwrap().print(indent)
    }

    #[test]
    fn content_moves_with_the_delimiter() {
        let block = "\"\"\"\n          deep\n      shallow\n   \"\"\"";
        assert_eq!(
            reprint(block, 2),
            "\"\"\"\n         deep\n     shallow\n  \"\"\""
        );
        assert!(!TextBlock::parse(block).unwrap().at_margin);
        // Tabs count as one character each, as they do to the compiler.
        assert_eq!(
            reprint("\"\"\"\n\t\ta\n\t\t\tb\n\t\t\"\"\"", 1),
            "\"\"\"\n a\n \tb\n \"\"\""
        );
    }

    #[test]
    fn closing_delimiter_gets_its_own_line() {
        let block = "\"\"\"\n    a\n    b\"\"\"";
        assert_eq!(reprint(block, 0), "\"\"\"\na\nb\\\n\"\"\"");
        assert_eq!(fingerprint(block), fingerprint(&reprint(block, 6)));
    }

    #[test]
    fn blank_lines_and_trailing_spaces() {
        let block = "\"\"\" \n    a  \n\n    b\n    \"\"\"";
        assert_eq!(reprint(block, 2), "\"\"\"\n  a\n\n  b\n  \"\"\"");
        assert_eq!(fingerprint(block), fingerprint(&reprint(block, 2)));
    }

    #[test]
    fn blocks_at_the_margin() {
        assert!(TextBlock::parse("\"\"\"\na\n  \"\"\"").unwrap().at_margin);
        assert!(TextBlock::parse("\"\"\"\n  a\n\"\"\"").unwrap().at_margin);
        assert!(!TextBlock::parse("\"\"\"\n  a\n  \"\"\"").unwrap().at_margin);
    }

    #[test]
    fn fingerprints_tell_values_apart() {
        let a = "\"\"\"\n  a\n  \"\"\"";
        assert_ne!(fingerprint(a), fingerprint("\"\"\"\n  a\n \"\"\""));
        assert_ne!(fingerprint(a), fingerprint("\"\"\"\n  a\"\"\""));
        assert_ne!(fingerprint(a), fingerprint("\"\"\"\n   a\n  \"\"\""));
        // An escaped backslash before the line break is no continuation.
        assert_ne!(
            fingerprint("\"\"\"\n  a\\\\\n  b\"\"\""),
            fingerprint("\"\"\"\n  a\\\\b\"\"\"")
        );
        assert_eq!(
            fingerprint("\"\"\"\n  a\\\n  b\"\"\""),
            fingerprint("\"\"\"\n  ab\"\"\"")
        );
    }
}
