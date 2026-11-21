//! Turns a [`Doc`] into text, choosing which groups to break.

use crate::doc::Doc;

/// Line breaks are not written when requested but held until the next text
/// arrives. That makes `HardLine` and `BlankLine` idempotent, lets the next
/// text's own indentation apply, and means lines never end in whitespace.
struct Output {
    text: String,
    column: usize,
    /// Line breaks owed before the next text: 0, 1, or 2 (a blank line).
    pending: u8,
}

impl Output {
    fn newline(&mut self, count: u8) {
        // Nothing has been printed yet: files do not start with blank lines.
        if !self.text.is_empty() {
            self.pending = self.pending.max(count);
        }
    }

    /// Writes a space, unless the next text will start a line.
    fn space(&mut self, indent: usize) {
        if self.pending == 0 && !self.text.is_empty() {
            self.write(" ", indent);
        }
    }

    fn write(&mut self, text: &str, indent: usize) {
        if self.pending > 0 {
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
    };
    // (indentation, printed flat?, document), last entry printed first.
    let mut stack: Vec<(usize, bool, &Doc)> = vec![(0, false, doc)];

    while let Some((indent, flat, doc)) = stack.pop() {
        match doc {
            Doc::Text(text) | Doc::Suffix(text) => out.write(text, indent),
            Doc::Space => out.space(indent),
            Doc::Line if flat => out.space(indent),
            Doc::SoftLine if flat => {}
            Doc::Line | Doc::SoftLine | Doc::HardLine => out.newline(1),
            Doc::BlankLine => out.newline(2),
            Doc::Concat(docs, _) => {
                stack.extend(docs.iter().rev().map(|doc| (indent, flat, doc)));
            }
            Doc::Indent(columns, doc, _) => stack.push((indent + *columns as usize, flat, doc)),
            Doc::Group(doc, forced) => {
                let flat = flat
                    || (!forced && {
                        let column = if out.pending > 0 { indent } else { out.column };
                        fits(doc, &stack, line_width as isize - column as isize)
                    });
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
            Doc::Indent(_, doc, _) => stack.push((flat, doc)),
            Doc::Group(doc, forced) => stack.push((flat && !forced, doc)),
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
