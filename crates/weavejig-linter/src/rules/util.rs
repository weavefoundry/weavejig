//! Helpers shared by rules that are about text and edits rather than syntax.

use weavejig_syntax::{TextRange, TextSize};

use crate::{Context, Edit};

/// An edit that deletes `range`. If nothing else is on the lines the range
/// covers, the lines go too, so that no blank line is left behind.
pub(crate) fn delete_lines(ctx: &Context<'_>, range: TextRange) -> Edit {
    let text = ctx.text;
    let (start, end) = (usize::from(range.start()), usize::from(range.end()));
    let line_start = text[..start].rfind(['\n', '\r']).map_or(0, |i| i + 1);
    let rest = &text[end..];
    let line_len = rest.find(['\n', '\r']).unwrap_or(rest.len());
    let alone = text[line_start..start].trim().is_empty() && rest[..line_len].trim().is_empty();
    if !alone {
        return Edit::delete(range);
    }
    let terminator = match &rest.as_bytes()[line_len..] {
        [b'\r', b'\n', ..] => 2,
        [b'\r' | b'\n', ..] => 1,
        _ => 0,
    };
    let to = |offset: usize| TextSize::try_from(offset).unwrap_or_default();
    Edit::delete(TextRange::new(
        to(line_start),
        to(end + line_len + terminator),
    ))
}

/// The number of columns `line` takes when each tab moves to the next
/// multiple of `tab_width`.
pub(crate) fn expanded_width(line: &str, tab_width: usize) -> usize {
    let mut width = 0;
    for c in line.chars() {
        if c == '\t' && tab_width > 0 {
            width = (width / tab_width + 1) * tab_width;
        } else {
            width += 1;
        }
    }
    width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_expand_to_stops() {
        assert_eq!(expanded_width("abc", 8), 3);
        assert_eq!(expanded_width("\tabc", 8), 11);
        assert_eq!(expanded_width("ab\tc", 8), 9);
        assert_eq!(expanded_width("ab\tc", 4), 5);
        assert_eq!(expanded_width("é\t", 8), 8);
    }
}
