//! Turns a [`Doc`] into text, choosing which groups to break.

use std::collections::HashSet;

use crate::doc::{Doc, Tag};

/// Line breaks are not written when requested but held until the next text
/// arrives. That makes `HardLine` and `BlankLine` idempotent and means lines
/// never end in whitespace. The new line is indented as the most recent line
/// break asked, not as the text that follows it: the text may sit in an
/// indented document of its own that only concerns its later lines.
struct Output {
    text: String,
    column: usize,
    /// Line breaks owed before the next text: 0, 1, or 2 (a blank line).
    pending: u8,
    /// Indentation of the line the next text starts, if `pending > 0`.
    pending_indent: usize,
}

impl Output {
    fn newline(&mut self, count: u8, indent: usize) {
        // Nothing has been printed yet: files do not start with blank lines.
        if !self.text.is_empty() {
            self.pending = self.pending.max(count);
            self.pending_indent = indent;
        }
    }

    /// Writes a space, unless the next text will start a line.
    fn space(&mut self) {
        if self.pending == 0 && !self.text.is_empty() {
            self.write(" ");
        }
    }

    fn write(&mut self, text: &str) {
        if self.pending > 0 {
            let indent = self.pending_indent;
            let trimmed = self.text.trim_end_matches([' ', '\t']).len();
            self.text.truncate(trimmed);
            for _ in 0..self.pending {
                self.text.push('\n');
            }
            for _ in 0..indent {
                self.text.push(' ');
            }
            self.column = indent;
            self.pending = 0;
        }
        self.text.push_str(text);
        match text.rfind('\n') {
            Some(newline) => self.column = width(&text[newline + 1..]),
            None => self.column += width(text),
        }
    }
}

fn width(text: &str) -> usize {
    text.chars().count()
}

pub(crate) fn print(doc: &Doc, line_width: usize) -> String {
    let mut out = Output {
        text: String::new(),
        column: 0,
        pending: 0,
        pending_indent: 0,
    };
    // (indentation, printed flat?, document), last entry printed first.
    let mut stack: Vec<(usize, bool, &Doc)> = vec![(0, false, doc)];
    // Tagged groups that were broken.
    let mut broken: HashSet<Tag> = HashSet::new();

    while let Some((indent, flat, doc)) = stack.pop() {
        let whole = doc;
        match doc {
            Doc::Text(text) | Doc::Suffix(text) => out.write(text),
            Doc::Space => out.space(),
            // A line break that a comment already forced takes the place of
            // this one, at this one's indentation.
            Doc::Line | Doc::SoftLine if flat && out.pending > 0 => out.pending_indent = indent,
            Doc::Line if flat => out.space(),
            Doc::SoftLine if flat => {}
            Doc::Line | Doc::SoftLine | Doc::HardLine => out.newline(1, indent),
            Doc::BlankLine => out.newline(2, indent),
            Doc::Concat(docs, _) => {
                stack.extend(docs.iter().rev().map(|doc| (indent, flat, doc)));
            }
            Doc::Indent(columns, doc, _) => stack.push((indent + *columns as usize, flat, doc)),
            Doc::IndentIf(tag, columns, doc, _) => {
                let extra = if broken.contains(tag) {
                    *columns as usize
                } else {
                    0
                };
                stack.push((indent + extra, flat, doc));
            }
            Doc::FillBreak(doc, _) => {
                let fits_here = flat
                    || (out.pending == 0
                        && fits(doc, &stack, line_width as isize - out.column as isize - 1));
                if fits_here {
                    out.space();
                } else {
                    out.newline(1, indent);
                }
                stack.push((indent, flat, doc));
            }
            Doc::GroupIf(tag, doc, forced) => {
                let flat = flat
                    || (!forced && !broken.contains(tag) && {
                        let column = if out.pending > 0 {
                            out.pending_indent
                        } else {
                            out.column
                        };
                        fits(doc, &stack, line_width as isize - column as isize)
                    });
                stack.push((indent, flat, doc));
            }
            Doc::Group(doc, forced) | Doc::TaggedGroup(_, doc, forced) => {
                let flat = flat
                    || (!forced && {
                        let column = if out.pending > 0 {
                            out.pending_indent
                        } else {
                            out.column
                        };
                        fits(doc, &stack, line_width as isize - column as isize)
                    });
                if let (false, Doc::TaggedGroup(tag, ..)) = (flat, whole) {
                    broken.insert(*tag);
                }
                stack.push((indent, flat, doc));
            }
        }
    }

    let trimmed = out.text.trim_end().len();
    out.text.truncate(trimmed);
    if !out.text.is_empty() {
        out.text.push('\n');
    }
    out.text
}

/// Whether `doc`, printed flat, fits in `remaining` columns together with
/// whatever follows it on the same line. `rest` is the printer's stack: the
/// documents still to come, each in the mode already chosen for it.
fn fits(doc: &Doc, rest: &[(usize, bool, &Doc)], mut remaining: isize) -> bool {
    let mut rest = rest.iter().rev();
    let mut stack: Vec<(bool, &Doc)> = vec![(true, doc)];
    loop {
        let (flat, doc) = match stack.pop() {
            Some(entry) => entry,
            None => match rest.next() {
                Some((_, flat, doc)) => (*flat, *doc),
                None => return true,
            },
        };
        match doc {
            Doc::Text(text) => match text.find('\n') {
                Some(newline) => return remaining >= width(&text[..newline]) as isize,
                None => remaining -= width(text) as isize,
            },
            Doc::Suffix(_) => {}
            Doc::Space => remaining -= 1,
            Doc::Line if flat => remaining -= 1,
            Doc::SoftLine if flat => {}
            // A line break ends the line: everything measured so far fits.
            Doc::Line | Doc::SoftLine | Doc::HardLine | Doc::BlankLine => return true,
            Doc::Concat(docs, _) => stack.extend(docs.iter().rev().map(|doc| (flat, doc))),
            Doc::Indent(_, doc, _) | Doc::IndentIf(_, _, doc, _) => stack.push((flat, doc)),
            Doc::Group(doc, forced)
            | Doc::TaggedGroup(_, doc, forced)
            | Doc::GroupIf(_, doc, forced) => stack.push((flat && !forced, doc)),
            Doc::FillBreak(doc, _) if flat => {
                remaining -= 1;
                stack.push((flat, doc));
            }
            // The next item of a fill starts a new line if need be.
            Doc::FillBreak(..) => return true,
        }
        if remaining < 0 {
            return false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::*;

    fn call(args: &[&str]) -> Doc {
        let args = join_lines(args.iter().map(|arg| text(*arg)).collect(), ",");
        concat(vec![
            text("function("),
            group(indent(4, concat(vec![Doc::SoftLine, group(args)]))),
            text(");"),
        ])
    }

    #[test]
    fn group_stays_flat_when_it_fits() {
        // 27 columns wide.
        assert_eq!(
            print(&call(&["aaaa", "bbbb", "cccc"]), 27),
            "function(aaaa, bbbb, cccc);\n"
        );
    }

    #[test]
    fn outer_group_breaks_before_inner() {
        assert_eq!(
            print(&call(&["aaaa", "bbbb", "cccc"]), 26),
            "function(\n    aaaa, bbbb, cccc);\n"
        );
        assert_eq!(
            print(&call(&["aaaa", "bbbb", "cccc"]), 21),
            "function(\n    aaaa,\n    bbbb,\n    cccc);\n"
        );
    }

    #[test]
    fn text_after_a_group_counts_towards_its_width() {
        // "aaa bbb" fits in 7 columns, but not with the ";" that follows.
        let doc = concat(vec![
            group(concat(vec![text("aaa"), Doc::Line, text("bbb")])),
            text(";"),
        ]);
        assert_eq!(print(&doc, 8), "aaa bbb;\n");
        assert_eq!(print(&doc, 7), "aaa\nbbb;\n");
    }

    #[test]
    fn hard_lines_are_idempotent_and_force_breaks() {
        let doc = concat(vec![
            text("{"),
            indent(
                2,
                concat(vec![
                    Doc::HardLine,
                    Doc::HardLine,
                    text("a;"),
                    Doc::BlankLine,
                    Doc::HardLine,
                    text("b;"),
                ]),
            ),
            Doc::HardLine,
            text("}"),
        ]);
        assert_eq!(print(&doc, 80), "{\n  a;\n\n  b;\n}\n");

        let forced = group(concat(vec![
            text("a"),
            Doc::Line,
            text("b"),
            Doc::HardLine,
            text("c"),
        ]));
        assert_eq!(print(&forced, 80), "a\nb\nc\n");
    }

    #[test]
    fn suffixes_take_no_room() {
        let doc = concat(vec![
            group(concat(vec![text("aaaa"), Doc::Line, text("bbbb")])),
            Doc::Suffix(" // a long trailing comment".to_string()),
            Doc::HardLine,
            text("c"),
        ]);
        assert_eq!(print(&doc, 10), "aaaa bbbb // a long trailing comment\nc\n");
    }

    #[test]
    fn no_trailing_whitespace_or_leading_blank_lines() {
        let doc = concat(vec![
            Doc::BlankLine,
            text("a "),
            Doc::HardLine,
            text("b"),
            Doc::BlankLine,
        ]);
        assert_eq!(print(&doc, 80), "a\nb\n");
    }
}
