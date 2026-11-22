//! Long string literals.
//!
//! google-java-format re-splits a string literal that runs past the line
//! width into a concatenation of shorter literals, one per line:
//!
//! ```text
//!     foo(
//!         "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt"
//!             + " ut labore et dolore magna aliqua.");
//! ```
//!
//! It does this to the formatted text, after layout: the new lines get their
//! own indentation and nothing else on the page moves. [`reflow`] does the
//! same. The rules, as observed:
//!
//! - A *run* is a maximal sequence of string literals joined by `+`, or a
//!   lone string literal; a comment between two literals ends a run. A run
//!   is rewritten when one of its literals is on a line longer than the
//!   width, and its literals are first merged into one.
//! - The text breaks before a space, which starts the next piece, and after
//!   the last of one or more `\n` or `\r` escapes. Each piece takes as many
//!   words as fit. A piece shorter than five characters is never broken off
//!   at a space; a word that does not fit anywhere runs past the width.
//! - A word too long for any line may instead be cut where one of the
//!   merged literals started, if the part before that fits on the line. A
//!   run with no space or line break in it at all is left as it is.
//! - The last piece must fit together with whatever follows the run on its
//!   line (`);`, say). When the rest of the text would fit on a line but not
//!   with that, the line is filled only up to where it does.
//! - The first piece stays where the run started. The `+` of each further
//!   piece goes under the `+` before the run, or four columns to the right
//!   of the run if it starts its expression.
//! - A literal whose meaning would change is left alone: the receiver of a
//!   method call or field access, a cast's operand, and the like.

use std::collections::HashMap;

use weavejig_syntax::{
    SyntaxKind::*, SyntaxNode, SyntaxToken, TextSize, parse, significant_children,
};

/// The shortest piece that is broken off at a space.
const MIN_PIECE: usize = 5;

/// A maximal sequence of string literals joined by `+`: consecutive operands
/// of one additive chain, or a lone string literal.
pub(crate) struct Run {
    pub(crate) literals: Vec<SyntaxToken>,
    /// Whether the run can be replaced by another concatenation of literals
    /// with the same value without changing what the code means. It cannot
    /// when it is the operand of something that binds tighter than `+`.
    pub(crate) splittable: bool,
    /// The `+` before the run, if the run is not the first operand.
    pub(crate) plus_before: Option<SyntaxToken>,
}

/// The token of a string literal expression.
fn string_literal(node: &SyntaxNode) -> Option<SyntaxToken> {
    if node.kind() != LITERAL {
        return None;
    }
    let mut children = significant_children(node);
    let token = children.next()?.into_token()?;
    (token.kind() == STRING_LITERAL && children.next().is_none()).then_some(token)
}

/// The operator of a `+` or `-` expression.
fn additive_operator(node: &SyntaxNode) -> Option<SyntaxToken> {
    if node.kind() != BINARY_EXPR {
        return None;
    }
    significant_children(node)
        .filter_map(|element| element.into_token())
        .find(|token| matches!(token.kind(), PLUS | MINUS))
}

/// Whether a lone string literal that is a child of `parent` may become a
/// concatenation.
fn splittable_in(parent: &SyntaxNode) -> bool {
    !matches!(
        parent.kind(),
        FIELD_ACCESS
            | METHOD_CALL
            | METHOD_REF
            | ARRAY_ACCESS
            | PREFIX_EXPR
            | POSTFIX_EXPR
            | CAST_EXPR
            | BINARY_EXPR
            | INSTANCEOF_EXPR
            | ERROR
    )
}

/// Every run of string literals in the tree, in no particular order.
pub(crate) fn runs(root: &SyntaxNode) -> Vec<Run> {
    let mut runs = Vec::new();
    for node in root.descendants() {
        if let Some(literal) = string_literal(&node) {
            let parent = node.parent();
            if parent
                .as_ref()
                .is_some_and(|parent| additive_operator(parent).is_some())
            {
                continue; // Part of a chain, handled at the chain's top.
            }
            runs.push(Run {
                literals: vec![literal],
                splittable: parent.is_none_or(|parent| splittable_in(&parent)),
                plus_before: None,
            });
        } else if additive_operator(&node).is_some() {
            let continues_chain = node.parent().is_some_and(|parent| {
                additive_operator(&parent).is_some()
                    && parent.first_child().is_some_and(|left| left == node)
            });
            if !continues_chain {
                chain_runs(&node, &mut runs);
            }
        }
    }
    runs
}

/// The runs among the operands of the additive chain whose top is `top`.
fn chain_runs(top: &SyntaxNode, runs: &mut Vec<Run>) {
    // Walk down the left spine; the chain is `operands` joined by `operators`.
    let mut tail = Vec::new();
    let mut current = top.clone();
    let first = loop {
        let (Some(left), Some(operator), Some(right)) = (
            current.first_child(),
            additive_operator(&current),
            current.last_child(),
        ) else {
            return;
        };
        tail.push((operator, right));
        if additive_operator(&left).is_some() {
            current = left;
        } else {
            break left;
        }
    };
    let mut operands = vec![first];
    let mut operators = Vec::new();
    for (operator, operand) in tail.into_iter().rev() {
        operators.push(operator);
        operands.push(operand);
    }

    let mut i = 0;
    while i < operands.len() {
        // A literal after `-` is not joined to anything.
        let joined = i == 0 || operators[i - 1].kind() == PLUS;
        let Some(literal) = string_literal(&operands[i]).filter(|_| joined) else {
            i += 1;
            continue;
        };
        let mut literals = vec![literal];
        let mut j = i + 1;
        while j < operands.len() && operators[j - 1].kind() == PLUS {
            let Some(literal) = string_literal(&operands[j]) else {
                break;
            };
            literals.push(literal);
            j += 1;
        }
        runs.push(Run {
            literals,
            splittable: operators.get(j - 1).is_none_or(|op| op.kind() == PLUS),
            plus_before: (i > 0).then(|| operators[i - 1].clone()),
        });
        i = j;
    }
}

/// The text between a string literal's quotes.
pub(crate) fn content(literal: &str) -> Option<&str> {
    literal
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
}

/// The UTF-16 code units that the content of a string literal stands for,
/// or `None` if it contains an invalid escape (JLS §3.3, §3.10.7).
pub(crate) fn value(content: &str) -> Option<Vec<u16>> {
    // Unicode escapes come first. A backslash can start one only if an even
    // number of backslashes precede it.
    let raw: Vec<char> = content.chars().collect();
    let mut units = Vec::with_capacity(raw.len());
    let mut backslashes = 0;
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == '\\' && backslashes % 2 == 0 && raw.get(i + 1) == Some(&'u') {
            let mut j = i + 1;
            while raw.get(j) == Some(&'u') {
                j += 1;
            }
            let hex: String = raw.get(j..j + 4)?.iter().collect();
            units.push(u16::from_str_radix(&hex, 16).ok()?);
            backslashes = 0;
            i = j + 4;
            continue;
        }
        backslashes = if raw[i] == '\\' { backslashes + 1 } else { 0 };
        let mut buffer = [0; 2];
        units.extend_from_slice(raw[i].encode_utf16(&mut buffer));
        i += 1;
    }

    let mut value = Vec::with_capacity(units.len());
    let mut i = 0;
    let is_octal = |unit: Option<&u16>| unit.is_some_and(|&unit| (u('0')..=u('7')).contains(&unit));
    while i < units.len() {
        if units[i] != u('\\') {
            value.push(units[i]);
            i += 1;
            continue;
        }
        let escape = *units.get(i + 1)?;
        i += 2;
        let unit = match char::from_u32(u32::from(escape))? {
            'b' => 8,
            't' => 9,
            'n' => 10,
            'f' => 12,
            'r' => 13,
            's' => 32,
            '"' => 34,
            '\'' => 39,
            '\\' => 92,
            '0'..='7' => {
                let max_digits = if escape <= u('3') { 3 } else { 2 };
                let mut code = escape - u('0');
                let mut digits = 1;
                while digits < max_digits && is_octal(units.get(i)) {
                    code = code * 8 + (units[i] - u('0'));
                    i += 1;
                    digits += 1;
                }
                code
            }
            _ => return None,
        };
        value.push(unit);
    }
    Some(value)
}

fn u(c: char) -> u16 {
    c as u16
}

/// The value of several literal contents concatenated.
fn concat_value<'a>(contents: impl IntoIterator<Item = &'a str>) -> Option<Vec<u16>> {
    let mut all = Vec::new();
    for content in contents {
        all.extend(value(content)?);
    }
    Some(all)
}

/// The length in bytes of the character or escape sequence that starts
/// `rest`.
fn atom_len(rest: &str) -> usize {
    let mut chars = rest.chars();
    let Some(first) = chars.next() else {
        return 0;
    };
    if first != '\\' {
        return first.len_utf8();
    }
    match chars.next() {
        Some('u') => {
            let us = rest[1..].bytes().take_while(|&b| b == b'u').count();
            (1 + us + 4).min(rest.len())
        }
        Some(d @ '0'..='7') => {
            let max_digits = if d <= '3' { 3 } else { 2 };
            1 + rest[1..]
                .bytes()
                .take(max_digits)
                .take_while(|b| (b'0'..=b'7').contains(b))
                .count()
        }
        Some(c) => 1 + c.len_utf8(),
        None => 1,
    }
}

/// A word of a string being split: where it is in the text, how wide it is,
/// whether the string must break after it, and where inside it one of the
/// literals that were merged started (with the width before that point).
#[derive(Clone)]
struct Word {
    start: usize,
    end: usize,
    width: usize,
    hard_break: bool,
    cuts: Vec<(usize, usize)>,
}

/// Cuts the content of a string literal into words: a new word starts at
/// every space, and after the last of one or more `\n` or `\r` escapes.
/// `boundaries` are where the literals merged into `content` started.
fn words(content: &str, boundaries: &[usize]) -> Vec<Word> {
    let mut words: Vec<Word> = Vec::new();
    let mut word = Word {
        start: 0,
        end: 0,
        width: 0,
        hard_break: false,
        cuts: Vec::new(),
    };
    let mut end_word = |word: &mut Word, at: usize, hard_break: bool| {
        let next = Word {
            start: at,
            end: at,
            width: 0,
            hard_break: false,
            cuts: Vec::new(),
        };
        let mut done = std::mem::replace(word, next);
        done.end = at;
        done.hard_break = hard_break;
        words.push(done);
    };
    let line_break = |atom: &str| matches!(atom, "\\n" | "\\r");
    let mut i = 0;
    while i < content.len() {
        let len = atom_len(&content[i..]);
        let atom = &content[i..i + len];
        if i > word.start {
            if atom == " " {
                end_word(&mut word, i, false);
            } else if boundaries.contains(&i) {
                word.cuts.push((i, word.width));
            }
        }
        word.width += atom.chars().count();
        i += len;
        if line_break(atom)
            && i < content.len()
            && !line_break(&content[i..i + atom_len(&content[i..])])
        {
            end_word(&mut word, i, true);
        }
    }
    if i > word.start {
        end_word(&mut word, i, false);
    }
    words
}

/// The columns a run of literals is split for: where the first piece
/// starts, where the others start, and how much follows the last.
#[derive(Clone, Copy)]
struct Columns {
    first: usize,
    rest: usize,
    trailing: usize,
}

/// Splits `content` into pieces for `columns`. Pieces break between
/// [`words`]. A word too long for any line is cut where one of the merged
/// literals started, if the part before that still fits on this one.
fn split<'a>(
    content: &'a str,
    boundaries: &[usize],
    columns: &Columns,
    width: usize,
) -> Vec<&'a str> {
    let mut words = words(content, boundaries);
    let mut pieces = Vec::new();
    let mut column = columns.first;
    let mut i = 0;
    while i < words.len() {
        let segment_end = words[i..]
            .iter()
            .position(|word| word.hard_break)
            .map_or(words.len(), |k| i + k + 1);
        // Two columns for the quotes.
        let remaining = 2 + words[i..segment_end].iter().map(|w| w.width).sum::<usize>();
        let mut limit = width;
        if segment_end == words.len()
            && column + remaining <= width
            && column + remaining + columns.trailing > width
        {
            limit = width.saturating_sub(columns.trailing);
        }
        let start = words[i].start;
        let mut end = start;
        let mut len = 0;
        while i < segment_end {
            let word = &words[i];
            if column + 2 + len + word.width <= limit {
                len += word.width;
                end = word.end;
                i += 1;
                continue;
            }
            // A word that would fit on a line of its own moves there whole.
            let fits_alone = columns.rest + 2 + word.width <= width;
            let cut = word.cuts.iter().rev().copied().find(|&(_, before)| {
                !fits_alone && column + 2 + len + before <= limit && len + before >= MIN_PIECE
            });
            if let Some((at, before)) = cut {
                end = at;
                let rest = &mut words[i];
                rest.start = at;
                rest.width -= before;
                rest.cuts = rest
                    .cuts
                    .iter()
                    .filter(|&&(offset, _)| offset > at)
                    .map(|&(offset, w)| (offset, w - before))
                    .collect();
                break;
            }
            if len >= MIN_PIECE {
                break;
            }
            // Too short to break off: the word joins this piece anyway.
            len += word.width;
            end = word.end;
            i += 1;
        }
        pieces.push(&content[start..end]);
        column = columns.rest;
    }
    pieces
}

/// Byte offsets of the lines of `text` that are wider than `width`.
fn long_lines(text: &str, width: usize) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    let mut start = 0;
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        if body.chars().count() > width {
            lines.push((start, start + body.len()));
        }
        start += line.len();
    }
    lines
}

/// The column of byte offset `offset` in `text`.
fn column(text: &str, offset: usize) -> usize {
    let line_start = text[..offset].rfind('\n').map_or(0, |i| i + 1);
    text[line_start..offset].chars().count()
}

/// `formatted` with every long string literal split as google-java-format
/// splits it, or `None` if nothing changes.
pub(crate) fn reflow(formatted: &str, width: usize) -> Option<String> {
    let long = long_lines(formatted, width);
    if !long
        .iter()
        .any(|&(start, end)| formatted[start..end].contains('"'))
    {
        return None;
    }
    let on_long_line = |offset: usize| {
        long.binary_search_by(|&(start, end)| {
            if end < offset {
                std::cmp::Ordering::Less
            } else if start > offset {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
    };

    let parse = parse(formatted);
    if !parse.is_ok() {
        return None;
    }
    let mut edits = Vec::new();
    for run in runs(&parse.syntax()) {
        if !run.splittable {
            continue;
        }
        for run in without_comments(run) {
            let long = run
                .literals
                .iter()
                .any(|literal| on_long_line(usize::from(literal.text_range().start())));
            if long && let Some(edit) = reflow_run(formatted, &run, width) {
                edits.push(edit);
            }
        }
    }
    if edits.is_empty() {
        return None;
    }
    edits.sort_by_key(|&(start, _, _)| start);
    let mut out = String::with_capacity(formatted.len() + edits.len() * 16);
    let mut done = 0;
    for (start, end, replacement) in edits {
        out.push_str(&formatted[done..start]);
        out.push_str(&replacement);
        done = end;
    }
    out.push_str(&formatted[done..]);
    Some(out)
}

/// `run` cut wherever a comment lies between two of its literals: the
/// comment stays, and the literals on either side are split apart.
fn without_comments(run: Run) -> Vec<Run> {
    let mut runs = Vec::new();
    let mut current = Run {
        literals: Vec::new(),
        splittable: run.splittable,
        plus_before: run.plus_before,
    };
    for literal in run.literals {
        if let Some(previous) = current.literals.last() {
            let mut plus = None;
            let mut commented = false;
            let mut token = previous.next_token();
            while let Some(t) = token.filter(|t| *t != literal) {
                commented |= t.kind().is_comment();
                if t.kind() == PLUS {
                    plus = Some(t.clone());
                }
                token = t.next_token();
            }
            if commented {
                let done = std::mem::replace(
                    &mut current,
                    Run {
                        literals: Vec::new(),
                        splittable: run.splittable,
                        plus_before: plus,
                    },
                );
                runs.push(done);
            }
        }
        current.literals.push(literal);
    }
    runs.push(current);
    runs
}

/// The replacement for one run: its byte range and new text.
fn reflow_run(text: &str, run: &Run, width: usize) -> Option<(usize, usize, String)> {
    let (first, last) = (run.literals.first()?, run.literals.last()?);
    let contents: Vec<&str> = run
        .literals
        .iter()
        .map(|literal| content(literal.text()))
        .collect::<Option<_>>()?;
    let merged = contents.concat();
    let boundaries: Vec<usize> = contents
        .iter()
        .scan(0, |offset, content| {
            *offset += content.len();
            Some(*offset)
        })
        .collect();
    // Text with nowhere to break is left as it is, however it was split.
    if words(&merged, &[]).len() < 2 {
        return None;
    }
    let expected = concat_value(contents.iter().copied())?;
    if value(&merged)? != expected {
        return None; // Merging would join escapes, as in `"\1" + "2"`.
    }

    let start = usize::from(first.text_range().start());
    let end = usize::from(last.text_range().end());
    let first_column = column(text, start);
    let plus_column = match &run.plus_before {
        Some(plus) => column(text, usize::from(plus.text_range().start())),
        None => first_column + 4,
    };
    let line_end = text[end..].find('\n').map_or(text.len(), |i| end + i);
    let trailing = text[end..line_end].trim_end_matches('\r').chars().count();

    let columns = Columns {
        first: first_column,
        rest: plus_column + 2,
        trailing,
    };
    let pieces = split(&merged, &boundaries, &columns, width);
    if pieces.is_empty() || pieces == contents || concat_value(pieces.iter().copied())? != expected
    {
        return None;
    }
    let mut replacement = String::new();
    for (i, piece) in pieces.iter().enumerate() {
        if i > 0 {
            replacement.push('\n');
            replacement.extend(std::iter::repeat_n(' ', plus_column));
            replacement.push_str("+ ");
        }
        replacement.push('"');
        replacement.push_str(piece);
        replacement.push('"');
    }
    Some((start, end, replacement))
}

/// For the safety check: each run's first offset, mapped to its end offset
/// and a token that stands for the whole run. Two runs get the same token
/// exactly when their literals spell the same text between the quotes once
/// joined, and so also have the same value.
pub(crate) fn run_tokens(root: &SyntaxNode) -> HashMap<TextSize, (TextSize, String)> {
    let mut tokens = HashMap::new();
    for run in runs(root) {
        let (Some(first), Some(last)) = (run.literals.first(), run.literals.last()) else {
            continue;
        };
        let contents: Option<Vec<&str>> = run
            .literals
            .iter()
            .map(|literal| content(literal.text()))
            .collect();
        let token = match contents {
            Some(contents) => format!(
                "\"{}\" {:?}",
                contents.concat(),
                concat_value(contents.iter().copied())
            ),
            None => run
                .literals
                .iter()
                .map(|literal| literal.text())
                .collect::<Vec<_>>()
                .join(" + "),
        };
        tokens.insert(first.text_range().start(), (last.text_range().end(), token));
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pieces(content: &str, first: usize, rest: usize, trailing: usize) -> Vec<&str> {
        let columns = Columns {
            first,
            rest,
            trailing,
        };
        split(content, &[], &columns, 100)
    }

    #[test]
    fn values() {
        let utf16 = |s: &str| s.encode_utf16().collect::<Vec<_>>();
        assert_eq!(value(r"a\tb\n"), Some(utf16("a\tb\n")));
        assert_eq!(value(r"\1"), Some(vec![1]));
        assert_eq!(value(r"\12"), Some(vec![10]));
        assert_eq!(value(r"\400"), Some(vec![32, u('0')]));
        // Unicode escapes, spelled out so that nothing translates them here.
        let unicode = |hex: &str| format!("{}u{hex}", '\\');
        assert_eq!(
            value(&format!("{}{}", unicode("00e9"), unicode("u0041"))),
            Some(utf16("\u{e9}A"))
        );
        assert_eq!(
            value(&format!("\\{}", unicode("0041"))),
            Some(utf16(&format!("\\{}", &unicode("0041")[1..])))
        );
        assert_eq!(value(&format!("{}n", unicode("005c"))), Some(vec![10]));
        assert_eq!(value(r"\q"), None);
        assert_eq!(value("\u{1F600}"), Some(utf16("\u{1F600}")));
    }

    #[test]
    fn breaks_before_spaces_and_after_newlines() {
        let text = "ab ".repeat(40);
        let text = text.trim_end();
        assert_eq!(
            pieces(text, 8, 14, 2),
            [&text[..89], &text[89..]].as_slice()
        );
        assert_eq!(
            pieces(r"one\ntwo\n\nthree", 8, 14, 0),
            [r"one\n", r"two\n\n", "three"]
        );
        assert_eq!(
            pieces(r"one\r\ntwo\rthree", 8, 14, 0),
            [r"one\r\n", r"two\r", "three"]
        );
        assert!(pieces("", 8, 14, 0).is_empty());
        // Never between an escape's characters.
        let escaped = format!("{}\\\"{}", "a".repeat(88), " b");
        assert_eq!(pieces(&escaped, 8, 14, 0).len(), 2);
    }

    #[test]
    fn literals_that_were_joined_may_stay_apart() {
        let words = "ab ".repeat(20);
        let long = "a".repeat(95);
        let merged = format!("{words}{long}");
        let columns = Columns {
            first: 8,
            rest: 14,
            trailing: 2,
        };
        assert_eq!(split(&merged, &[60], &columns, 100), [&words, &long]);
        // A boundary is used only to cut a word too long for any line.
        let merged = format!("{words}ab cd");
        assert_eq!(split(&merged, &[60], &columns, 100), [&merged]);
        let merged = format!("{words}{}", "b".repeat(40));
        assert_eq!(
            split(&merged, &[60], &columns, 100),
            [&words[..59], &merged[59..]]
        );
        // But not if that leaves a piece too short.
        let merged = format!("ab {long}");
        assert_eq!(split(&merged, &[3], &columns, 100), [&merged]);
    }

    #[test]
    fn short_pieces_are_not_broken_off() {
        let long = "a".repeat(95);
        assert_eq!(pieces(&format!("abcd {long}"), 8, 14, 0).len(), 1);
        assert_eq!(pieces(&format!("abcde {long}"), 8, 14, 0).len(), 2);
    }

    #[test]
    fn the_last_piece_leaves_room_for_what_follows() {
        let text = "ab ".repeat(30);
        let text = text.trim_end(); // Ends at column 99 from column 8.
        assert_eq!(pieces(text, 8, 14, 0), [text]);
        assert_eq!(pieces(text, 8, 14, 7).len(), 2);
        assert_eq!(pieces(text, 8, 14, 7)[0].len(), 83);
    }
}
