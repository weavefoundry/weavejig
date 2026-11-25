//! Maps byte offsets to line and column numbers.

use crate::{TextRange, TextSize};

/// A 1-based line and column. Columns count Unicode scalar values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineCol {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone)]
pub struct LineIndex {
    /// Byte offset of the start of each line.
    line_starts: Vec<u32>,
}

impl LineIndex {
    pub fn new(text: &str) -> LineIndex {
        let mut line_starts = vec![0];
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\n' => line_starts.push(i as u32 + 1),
                // A lone `\r` is a line terminator in Java; `\r\n` counts once.
                b'\r' if bytes.get(i + 1) != Some(&b'\n') => line_starts.push(i as u32 + 1),
                _ => {}
            }
            i += 1;
        }
        LineIndex { line_starts }
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// The range of the 1-based `line` in `text`, without its terminator.
    /// Empty, at the end of the text, for a line past the last.
    pub fn line_range(&self, text: &str, line: u32) -> TextRange {
        let index = (line as usize).saturating_sub(1);
        let len = text.len() as u32;
        let Some(&start) = self.line_starts.get(index) else {
            return TextRange::empty(TextSize::new(len));
        };
        let next = self.line_starts.get(index + 1).copied().unwrap_or(len);
        let end = start
            + text[start as usize..next as usize]
                .trim_end_matches(['\n', '\r'])
                .len() as u32;
        TextRange::new(TextSize::new(start), TextSize::new(end))
    }

    /// The line and column of `offset`, which must lie on a character
    /// boundary of the `text` this index was built from.
    pub fn line_col(&self, text: &str, offset: TextSize) -> LineCol {
        let offset = u32::from(offset).min(text.len() as u32);
        let line = self.line_starts.partition_point(|&start| start <= offset) - 1;
        let line_start = self.line_starts[line] as usize;
        let column = text[line_start..offset as usize].chars().count() as u32;
        LineCol {
            line: line as u32 + 1,
            column: column + 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col() {
        let text = "ab\ncd\r\né f\rg";
        let index = LineIndex::new(text);
        assert_eq!(index.line_count(), 4);
        let at = |offset: u32| {
            let lc = index.line_col(text, TextSize::new(offset));
            (lc.line, lc.column)
        };
        assert_eq!(at(0), (1, 1));
        assert_eq!(at(2), (1, 3));
        assert_eq!(at(3), (2, 1));
        assert_eq!(at(7), (3, 1));
        // `é` is two bytes but one column.
        assert_eq!(at(10), (3, 3));
        assert_eq!(at(12), (4, 1));
    }

    #[test]
    fn line_ranges() {
        let text = "ab\ncd\r\n\né f\rg";
        let index = LineIndex::new(text);
        let line = |n: u32| &text[index.line_range(text, n)];
        assert_eq!(line(1), "ab");
        assert_eq!(line(2), "cd");
        assert_eq!(line(3), "");
        assert_eq!(line(4), "é f");
        assert_eq!(line(5), "g");
        assert_eq!(line(6), "");
    }
}
