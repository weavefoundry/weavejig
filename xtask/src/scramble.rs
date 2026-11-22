//! Rewrites the whitespace of a Java file into another plausible style, so
//! that the formatter can be scored on input that google-java-format has not
//! already formatted.
//!
//! Only whitespace between tokens changes: every token and comment is kept
//! byte for byte, in order. What changes, chosen per file:
//!
//! - indentation: kept, doubled, tabs, or none;
//! - where statements and expressions are wrapped: some line breaks inside a
//!   statement are joined, and some spaces become line breaks;
//! - braces: sometimes on a line of their own;
//! - stray trailing spaces.
//!
//! Blank lines and the line breaks around comments are kept, as they would be
//! in hand-written code. The choices are seeded by the file's contents, so a
//! file always scrambles the same way.

use weavejig_syntax::{SyntaxKind, lex};

/// Scrambles `text`. Returns it unchanged if it does not lex cleanly.
pub fn scramble(text: &str) -> String {
    let (tokens, errors) = lex(text);
    if !errors.is_empty() {
        return text.to_string();
    }
    let mut pieces = Vec::with_capacity(tokens.len());
    let mut offset = 0;
    for token in &tokens {
        let len = u32::from(token.len) as usize;
        pieces.push((token.kind, &text[offset..offset + len]));
        offset += len;
    }

    let mut rng = Rng::new(fnv1a(text.as_bytes()));
    let style = Style {
        indent: match rng.below(4) {
            0 => Indent::Keep,
            1 => Indent::Double,
            2 => Indent::Tabs,
            _ => Indent::None,
        },
        own_line_braces: rng.below(4) == 0,
        join_percent: [0, 30, 70][rng.below(3) as usize],
        break_percent: [0, 2, 5][rng.below(3) as usize],
        trailing_space_percent: [0, 5][rng.below(2) as usize],
    };

    let mut out = String::with_capacity(text.len() + text.len() / 8);
    // The indentation of the line being written, as it was in the input.
    let mut line_indent = 0;
    // Inside a `package` or `import` declaration. google-java-format rejects
    // a line break inside an import, and nobody writes one, so these keep
    // their spaces.
    let mut in_header = false;
    for (i, &(kind, piece)) in pieces.iter().enumerate() {
        match kind {
            SyntaxKind::IMPORT_KW | SyntaxKind::PACKAGE_KW => in_header = true,
            SyntaxKind::SEMI => in_header = false,
            _ => {}
        }
        if kind != SyntaxKind::WHITESPACE {
            out.push_str(piece);
            continue;
        }
        let prev = i.checked_sub(1).map(|j| pieces[j].0);
        let next = pieces.get(i + 1).map(|p| p.0);
        let (Some(prev), Some(next)) = (prev, next) else {
            // Leading or trailing whitespace of the file.
            out.push_str(piece);
            continue;
        };
        let newlines = piece.matches('\n').count();
        let near_comment = prev.is_comment() || next.is_comment();
        if in_header && newlines == 0 {
            out.push_str(piece);
            continue;
        }

        if newlines > 0 {
            line_indent = indent_width(&piece[piece.rfind('\n').unwrap_or(0) + 1..]);
            let inside_statement = !matches!(
                prev,
                SyntaxKind::SEMI | SyntaxKind::L_BRACE | SyntaxKind::R_BRACE
            ) && next != SyntaxKind::R_BRACE;
            if newlines == 1
                && !near_comment
                && !in_header
                && inside_statement
                && rng.percent(style.join_percent)
            {
                out.push(' ');
                continue;
            }
            for n in 0..newlines {
                // A space straight after a line comment would be part of it.
                let after_line_comment = n == 0 && prev == SyntaxKind::LINE_COMMENT;
                if !after_line_comment && rng.percent(style.trailing_space_percent) {
                    out.push(' ');
                }
                out.push('\n');
            }
            out.push_str(&style.indentation(line_indent));
        } else if next == SyntaxKind::L_BRACE && style.own_line_braces && !near_comment {
            out.push('\n');
            out.push_str(&style.indentation(line_indent));
        } else if !near_comment && rng.percent(style.break_percent) {
            out.push('\n');
            out.push_str(&style.indentation(line_indent + 4));
        } else {
            out.push_str(piece);
        }
    }
    out
}

#[derive(Clone, Copy)]
enum Indent {
    Keep,
    Double,
    Tabs,
    None,
}

struct Style {
    indent: Indent,
    own_line_braces: bool,
    join_percent: u64,
    break_percent: u64,
    trailing_space_percent: u64,
}

impl Style {
    fn indentation(&self, width: usize) -> String {
        match self.indent {
            Indent::Keep => " ".repeat(width),
            Indent::Double => " ".repeat(width * 2),
            Indent::Tabs => "\t".repeat(width / 2) + &" ".repeat(width % 2),
            Indent::None => String::new(),
        }
    }
}

/// Width of an indentation, counting a tab as two columns (one level of
/// google-java-format indentation).
fn indent_width(indent: &str) -> usize {
    indent.chars().map(|c| if c == '\t' { 2 } else { 1 }).sum()
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// SplitMix64: small, seedable, and the same on every platform.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn percent(&mut self, percent: u64) -> bool {
        percent > 0 && self.below(100) < percent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tokens other than whitespace, as text.
    fn significant(text: &str) -> Vec<String> {
        let (tokens, errors) = lex(text);
        assert!(errors.is_empty());
        let mut offset = 0;
        let mut out = Vec::new();
        for token in tokens {
            let len = u32::from(token.len) as usize;
            if token.kind != SyntaxKind::WHITESPACE {
                out.push(text[offset..offset + len].to_string());
            }
            offset += len;
        }
        out
    }

    const SAMPLE: &str = "\
package a;

import java.util.List;

/** Doc. */
import static java.util.Objects.requireNonNull;

class A {
  // A comment.
  int f(List<List<String>> xs, int y) {
    if (y > 0) {
      return xs.size()
          + y; // trailing
    }
    /* block */ return y >> 1;
  }
}
";

    #[test]
    fn keeps_every_token_and_comment() {
        // Different file contents give different seeds, so try a few.
        for extra in 0..50 {
            let input = format!("{SAMPLE}// {extra}\n");
            let output = scramble(&input);
            assert_eq!(significant(&input), significant(&output), "{output}");
        }
    }

    #[test]
    fn keeps_blank_lines_and_line_comment_ends() {
        for extra in 0..50 {
            let input = format!("{SAMPLE}// {extra}\n");
            let output = scramble(&input);
            let blank_line = output.contains("package a;\n\nimport")
                || output.contains("package a; \n\nimport")
                || output.contains("package a;\n \nimport")
                || output.contains("package a; \n \nimport");
            assert!(blank_line, "{output}");
            assert!(output.contains("// trailing\n"), "{output}");
            assert!(output.contains("// A comment.\n"), "{output}");
        }
    }

    #[test]
    fn keeps_imports_on_one_line() {
        for extra in 0..50 {
            let output = scramble(&format!("{SAMPLE}// {extra}\n"));
            assert!(output.contains("\nimport java.util.List;"), "{output}");
            assert!(
                output.contains("\nimport static java.util.Objects.requireNonNull;"),
                "{output}"
            );
        }
    }

    #[test]
    fn is_deterministic_and_varies() {
        assert_eq!(scramble(SAMPLE), scramble(SAMPLE));
        let distinct: std::collections::HashSet<String> = (0..50)
            .map(|extra| scramble(&format!("{SAMPLE}// {extra}\n")))
            .collect();
        assert!(distinct.len() > 10);
    }

    #[test]
    fn leaves_unlexable_text_alone() {
        let input = "class A { String s = \"unterminated; }\n";
        assert_eq!(scramble(input), input);
    }
}
