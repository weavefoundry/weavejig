//! Javadoc reflow.
//!
//! A `/** ... */` comment is split into words and the few HTML tags that
//! shape a Javadoc comment, and written out again: text wrapped at the line
//! width, paragraphs set off by `<p>`, block tags (`@param`) one per line
//! with a hanging indent, lists indented by nesting, and preformatted text
//! left as it is.
//!
//! The rules were worked out from what google-java-format prints for small
//! inputs; `tests/fixtures/reference/javadoc*.java` records those.

use std::borrow::Cow;

/// Whether a character separates words. A no-break space does not: it is
/// part of the word it is in, as in google-java-format.
fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\x0B' | '\x0C' | '\r')
}

/// Reflows a Javadoc comment that will be printed starting at `column`.
/// Returns its lines; every line after the first belongs at `column` too
/// (each already starts with the space that lines up the stars).
pub(crate) fn format(comment: &str, column: usize, line_width: usize) -> Vec<String> {
    let Some(body) = comment
        .strip_prefix("/**")
        .and_then(|rest| rest.strip_suffix("*/"))
    else {
        return comment.lines().map(str::to_string).collect();
    };
    let input = strip_stars(body);
    let tokens = tokenize(&input);

    let mut writer = Writer::new(line_width.saturating_sub(column + 3));
    for token in &tokens {
        writer.token(token);
    }
    let lines = writer.finish();

    if lines.is_empty() {
        return vec!["/** */".to_string()];
    }
    if let [only] = lines.as_slice()
        && !writer.has_block_tag
        && column + 4 + width(only) + 3 <= line_width
    {
        return vec![format!("/** {only} */")];
    }
    let mut out = vec!["/**".to_string()];
    for line in &lines {
        out.push(if line.is_empty() {
            " *".to_string()
        } else {
            format!(" * {line}")
        });
    }
    out.push(" */".to_string());
    out
}

fn width(text: &str) -> usize {
    text.chars().count()
}

/// The text of the comment without the `*` that starts each line, and
/// without the one space that usually follows it. Lines written without a
/// `*` lose the indentation they all share instead, so that preformatted
/// text among them keeps its shape.
fn strip_stars(body: &str) -> String {
    let starless = |line: &str| {
        !line.trim_matches(is_space).is_empty()
            && !line.trim_start_matches(is_space).starts_with('*')
    };
    let shared = body
        .lines()
        .skip(1)
        .filter(|line| starless(line))
        .map(|line| line.chars().take_while(|c| is_space(*c)).count())
        .min()
        .unwrap_or(0);
    let mut out = String::with_capacity(body.len());
    for (i, line) in body.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if i > 0 && starless(line) {
            let start = line
                .char_indices()
                .nth(shared)
                .map_or(line.len(), |(index, _)| index);
            out.push_str(line[start..].trim_end_matches(is_space));
            continue;
        }
        let line = if i == 0 {
            line
        } else {
            let line = line.trim_start_matches(is_space);
            line.strip_prefix('*').unwrap_or(line)
        };
        out.push_str(
            line.strip_prefix(' ')
                .unwrap_or(line)
                .trim_end_matches(is_space),
        );
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token<'a> {
    /// Text without whitespace in it, or several such joined by single
    /// spaces where no line may break: before a word that starts with `@`,
    /// which at the start of a line would read as a block tag.
    Word(Cow<'a, str>),
    /// A space that is dropped unless the line breaks there: the one after
    /// `<a href="...">`.
    OptionalBreak,
    /// `<code>`, what it holds exactly as written, and `</code>`. A line
    /// may break after the opening tag and before the closing one.
    Code {
        open: &'a str,
        content: &'a str,
        close: &'a str,
    },
    /// Whitespace within a paragraph.
    Space,
    /// Whitespace with an empty line in it.
    ParagraphBreak,
    /// `@param` and the like, at the start of a line.
    BlockTag(&'a str),
    /// `<p>`, with the tag as written if it has attributes.
    ParagraphOpen(Option<&'a str>),
    ParagraphClose,
    /// `<table>` or `{@snippet}` through its end, kept as written.
    Verbatim(&'a str),
    /// `<pre>`, what it holds exactly as written, and `</pre>`. A line may
    /// break after the opening tag and before the closing one.
    Pre {
        open: &'a str,
        content: &'a str,
        close: &'a str,
    },
    /// `<pre>{@code` on one line, the code, then `}</pre>`.
    PreCode {
        open: &'a str,
        code: &'a str,
    },
    ListOpen(&'a str),
    ListClose(&'a str),
    ItemOpen(&'a str),
    ItemClose,
    HeadingOpen(&'a str),
    HeadingClose(&'a str),
    QuoteOpen(&'a str),
    QuoteClose(&'a str),
    LineBreak,
    HtmlComment(&'a str),
}

/// Finds `needle` in `haystack` without regard to ASCII case.
fn find_ignore_case(haystack: &str, needle: &str) -> Option<usize> {
    let (haystack, needle) = (haystack.as_bytes(), needle.as_bytes());
    haystack
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
}

/// The index just past the `}` matching the `{` that `text` starts with.
fn matching_brace(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, byte) in text.bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// `<pre>` then `{@code` and a line break, the code, `}` and `</pre>`:
/// returns the opening tag and the code.
fn pre_code(region: &str) -> Option<(&str, &str)> {
    let open_end = region.find('>')? + 1;
    let (open, rest) = region.split_at(open_end);
    let rest = rest.trim_start_matches(is_space);
    let after_tag = rest.strip_prefix("{@code")?;
    let code_start = after_tag
        .find('\n')
        .filter(|&newline| after_tag[..newline].trim_matches(is_space).is_empty())?
        + 1;
    let block_end = matching_brace(rest)?;
    let tail = &rest[block_end..];
    let close = find_ignore_case(tail, "</pre>")?;
    if !tail[..close].trim_matches(is_space).is_empty() || tail.len() != close + "</pre>".len() {
        return None;
    }
    let code_offset = rest.len() - after_tag.len() + code_start;
    Some((open, &rest[code_offset..block_end - 1]))
}

/// The tag or other special construct that starts at the beginning of
/// `text`, and its length.
fn special(text: &str) -> Option<(Token<'_>, usize)> {
    if text.starts_with("<!--") {
        let end = text.find("-->")? + 3;
        return Some((Token::HtmlComment(&text[..end]), end));
    }
    if text.starts_with("{@snippet") {
        let end = matching_brace(text)?;
        return Some((Token::Verbatim(&text[..end]), end));
    }
    let rest = text.strip_prefix('<')?;
    let (closing, rest) = match rest.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, rest),
    };
    let name_len = rest
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(rest.len());
    if name_len == 0 {
        return None;
    }
    let name = rest[..name_len].to_ascii_lowercase();
    let after_name = &rest[name_len..];
    // Attributes may follow the name, but must be set off from it.
    if !after_name.starts_with(|c: char| c == '>' || c == '/' || is_space(c)) {
        return None;
    }
    let tag_len = text.len() - after_name.len() + after_name.find('>')? + 1;
    let tag = &text[..tag_len];
    let token = match (name.as_str(), closing) {
        ("p", false) => {
            let plain = after_name.starts_with('>')
                || after_name.trim_start_matches(is_space).starts_with("/>");
            Token::ParagraphOpen((!plain).then_some(tag))
        }
        ("p", true) => Token::ParagraphClose,
        ("code", false) => {
            let close = find_ignore_case(text, "</code>")?;
            let end = close + "</code>".len();
            let token = Token::Code {
                open: tag,
                content: &text[tag_len..close],
                close: &text[close..end],
            };
            return Some((token, end));
        }
        ("pre" | "table", false) => {
            let close = format!("</{name}>");
            let end = find_ignore_case(text, &close)? + close.len();
            let region = &text[..end];
            let token = match pre_code(region).filter(|_| name == "pre") {
                Some((open, code)) => Token::PreCode { open, code },
                None if name == "pre" => Token::Pre {
                    open: tag,
                    content: &region[tag_len..region.len() - close.len()],
                    close: &region[region.len() - close.len()..],
                },
                None => Token::Verbatim(region),
            };
            return Some((token, end));
        }
        ("ul" | "ol" | "dl", false) => Token::ListOpen(tag),
        ("ul" | "ol" | "dl", true) => Token::ListClose(tag),
        ("li" | "dt" | "dd", false) => Token::ItemOpen(tag),
        ("li" | "dt" | "dd", true) => Token::ItemClose,
        ("h1" | "h2" | "h3" | "h4" | "h5" | "h6", false) => Token::HeadingOpen(tag),
        ("h1" | "h2" | "h3" | "h4" | "h5" | "h6", true) => Token::HeadingClose(tag),
        ("blockquote", false) => Token::QuoteOpen(tag),
        ("blockquote", true) => Token::QuoteClose(tag),
        ("br", false) => Token::LineBreak,
        _ => return None,
    };
    Some((token, tag_len))
}

fn tokenize(input: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut at = 0;
    // Whether only whitespace separates `at` from the start of its line.
    let mut line_start = true;
    // The end of the inline tag (`{@code ...}`) that `at` is inside. What
    // an inline tag holds is text: no HTML and no block tags. An empty
    // line still starts a paragraph there.
    let mut inline_end = 0;
    while at < input.len() {
        let rest = &input[at..];
        let inline = at < inline_end;
        let blank = rest.len() - rest.trim_start_matches(is_space).len();
        if blank > 0 {
            let newlines = rest[..blank].matches('\n').count();
            tokens.push(if newlines >= 2 {
                Token::ParagraphBreak
            } else {
                Token::Space
            });
            line_start |= newlines > 0;
            at += blank;
            continue;
        }
        if line_start
            && !inline
            && rest.starts_with('@')
            && rest[1..].starts_with(|c: char| c.is_ascii_alphabetic())
        {
            let len = rest.find(is_space).unwrap_or(rest.len());
            tokens.push(Token::BlockTag(&rest[..len]));
            at += len;
            line_start = false;
            continue;
        }
        line_start = false;
        if !inline && let Some((token, len)) = special(rest) {
            tokens.push(token);
            at += len;
            continue;
        }
        // A word runs to the next whitespace or special construct.
        let mut len = 0;
        while len < rest.len() {
            let tail = &rest[len..];
            if at + len >= inline_end
                && tail.starts_with("{@")
                && !tail.starts_with("{@snippet")
                && let Some(end) = matching_brace(tail)
            {
                inline_end = at + len + end;
            }
            if tail.starts_with(is_space) {
                break;
            }
            if len > 0
                && at + len >= inline_end
                && (tail.starts_with('<') || tail.starts_with("{@snippet"))
                && special(tail).is_some()
            {
                break;
            }
            len += tail.chars().next().map_or(1, char::len_utf8);
        }
        let word = &rest[..len];
        at += len;
        // `see @Test`: no line breaks before a word that starts with `@`.
        if word.starts_with('@')
            && let [.., Token::Word(previous), Token::Space] = tokens.as_mut_slice()
        {
            let joined = format!("{previous} {word}");
            tokens.pop();
            tokens.pop();
            tokens.push(Token::Word(Cow::Owned(joined)));
            continue;
        }
        tokens.push(Token::Word(Cow::Borrowed(word)));
    }
    // `<a href="...">` and the link's text have no space between them,
    // unless the line breaks there.
    for i in 1..tokens.len() {
        if tokens[i] == Token::Space
            && let Token::Word(word) = &tokens[i - 1]
            && ends_link_tag(word)
        {
            tokens[i] = Token::OptionalBreak;
        }
    }
    tokens
}

/// Whether a word is the `href="...">` that ends an `<a>` tag.
fn ends_link_tag(word: &str) -> bool {
    word.find('>') == Some(word.len() - 1)
        && word
            .as_bytes()
            .get(..5)
            .is_some_and(|start| start.eq_ignore_ascii_case(b"href="))
}

/// Whitespace owed before the next thing written. Requests accumulate and
/// the largest wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Gap {
    None,
    /// Nothing, or a line break if what comes next does not fit.
    Soft,
    Space,
    Newline,
    BlankLine,
}

/// What was written last, as far as starting a paragraph is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Last {
    Nothing,
    Text,
    Block,
}

struct ListLevel {
    /// Indentation of the list's own tags.
    base: usize,
    in_item: bool,
}

struct Writer {
    lines: Vec<String>,
    current: String,
    available: usize,
    gap: Gap,
    wrote_anything: bool,
    lists: Vec<ListLevel>,
    /// Inside the text of a block tag, which hangs under it.
    in_block_tag: bool,
    has_block_tag: bool,
    last: Last,
    /// The source had an empty line since the last thing written.
    paragraph_break: bool,
    /// Whitespace is being skipped, after a tag that text attaches to.
    attach: bool,
    /// Only whitespace has come since a `</p>`: an empty line next to one
    /// starts no paragraph.
    after_paragraph_close: bool,
}

const ITEM_INDENT: usize = 2;
const HANGING_INDENT: usize = 4;

impl Writer {
    fn new(available: usize) -> Writer {
        Writer {
            lines: Vec::new(),
            current: String::new(),
            available,
            gap: Gap::None,
            wrote_anything: false,
            lists: Vec::new(),
            in_block_tag: false,
            has_block_tag: false,
            last: Last::Nothing,
            paragraph_break: false,
            attach: false,
            after_paragraph_close: false,
        }
    }

    /// Indentation of text that continues on a new line.
    fn indent(&self) -> usize {
        match self.lists.last() {
            Some(level) if level.in_item => level.base + ITEM_INDENT + HANGING_INDENT,
            Some(level) => level.base + ITEM_INDENT,
            None if self.in_block_tag => HANGING_INDENT,
            None => 0,
        }
    }

    fn request(&mut self, gap: Gap) {
        self.gap = self.gap.max(gap);
    }

    fn new_line(&mut self, indent: usize) {
        let line = std::mem::take(&mut self.current);
        self.lines.push(line.trim_end_matches(is_space).to_string());
        self.current = " ".repeat(indent);
    }

    /// Writes `text` after the whitespace owed, starting a line at `indent`
    /// if a line is started.
    fn write(&mut self, text: &str, indent: usize) {
        self.write_measured(text, indent, width(text));
    }

    /// [`Writer::write`], with `text` taking `columns` when it is decided
    /// whether it fits on the line.
    fn write_measured(&mut self, text: &str, indent: usize, columns: usize) {
        let mut gap = self.gap;
        if !self.wrote_anything {
            gap = Gap::None;
            self.current = " ".repeat(indent);
        }
        // Nested text is kept compact: no blank lines inside a list item
        // or a block tag.
        if gap == Gap::BlankLine && (!self.lists.is_empty() || self.in_block_tag) {
            gap = Gap::Newline;
        }
        match gap {
            Gap::BlankLine => {
                self.new_line(0);
                self.new_line(indent);
            }
            Gap::Newline => self.new_line(indent),
            Gap::Space => {
                if width(&self.current) + 1 + columns > self.available {
                    self.new_line(indent);
                } else {
                    self.current.push(' ');
                }
            }
            Gap::Soft => {
                if width(&self.current) + columns > self.available {
                    self.new_line(indent);
                }
            }
            Gap::None => {}
        }
        self.current.push_str(text);
        self.wrote_anything = true;
        self.gap = Gap::None;
    }

    fn token(&mut self, token: &Token<'_>) {
        if self.attach && matches!(token, Token::Space | Token::ParagraphBreak) {
            return;
        }
        self.attach = false;
        let starts_paragraph = std::mem::take(&mut self.paragraph_break);
        let after_paragraph_close = std::mem::take(&mut self.after_paragraph_close);
        let indent = self.indent();
        match token {
            Token::Space => {
                self.paragraph_break = starts_paragraph;
                self.after_paragraph_close = after_paragraph_close;
                self.request(Gap::Space);
            }
            Token::OptionalBreak => {
                self.paragraph_break = starts_paragraph;
                self.request(Gap::Soft);
            }
            Token::ParagraphBreak => {
                self.paragraph_break = !after_paragraph_close;
                self.after_paragraph_close = after_paragraph_close;
                self.request(Gap::Space);
            }
            Token::Code {
                open,
                content,
                close,
            } => {
                if starts_paragraph && self.last == Last::Text {
                    self.request(Gap::BlankLine);
                    self.write("<p>", indent);
                }
                self.write(open, indent);
                let mut lines = content.split('\n');
                if let Some(first) = lines.next().filter(|first| !first.is_empty()) {
                    self.gap = Gap::Soft;
                    self.write(first, indent);
                }
                for line in lines {
                    self.new_line(0);
                    self.current.push_str(line);
                }
                self.gap = Gap::Soft;
                self.write(close, indent);
                self.last = Last::Text;
            }
            Token::Word(word) => {
                // An empty line between two runs of text starts a paragraph.
                if starts_paragraph && self.last == Last::Text {
                    self.request(Gap::BlankLine);
                    self.write("<p>", indent);
                }
                self.write(word, indent);
                self.last = Last::Text;
            }
            Token::BlockTag(tag) => {
                self.lists.clear();
                // The first block tag is set off from the description; a
                // list or heading before any block tag is too.
                let gap = if !self.wrote_anything {
                    Gap::None
                } else if !self.has_block_tag {
                    Gap::BlankLine
                } else {
                    self.gap.max(Gap::Newline)
                };
                self.in_block_tag = false;
                self.gap = gap;
                self.write(tag, 0);
                self.in_block_tag = true;
                self.has_block_tag = true;
                self.last = Last::Text;
            }
            // A comment does not start with a paragraph mark.
            Token::ParagraphOpen(None) if !self.wrote_anything => self.attach = true,
            Token::ParagraphOpen(tag) => {
                self.request(Gap::BlankLine);
                self.write(tag.unwrap_or("<p>"), indent);
                self.attach = true;
                self.last = Last::Block;
            }
            Token::ParagraphClose => self.after_paragraph_close = true,
            Token::ItemClose => self.paragraph_break = starts_paragraph,
            Token::Verbatim(region) => {
                self.request(Gap::BlankLine);
                let mut lines = region.split('\n');
                self.write(lines.next().unwrap_or_default(), indent);
                for line in lines {
                    self.new_line(0);
                    self.current.push_str(line);
                }
                self.request(Gap::BlankLine);
                self.last = Last::Block;
            }
            Token::Pre {
                open,
                content,
                close,
            } => {
                self.request(Gap::BlankLine);
                self.write(open, indent);
                let mut lines = content.split('\n');
                if let Some(first) = lines.next().filter(|first| !first.is_empty()) {
                    self.gap = Gap::Soft;
                    self.write(first, indent);
                }
                for line in lines {
                    self.new_line(0);
                    self.current.push_str(line);
                }
                self.gap = Gap::Soft;
                self.write(close, indent);
                self.request(Gap::BlankLine);
                self.last = Last::Block;
            }
            Token::PreCode { open, code } => {
                self.request(Gap::BlankLine);
                self.write(&format!("{open}{{@code"), indent);
                let mut lines: Vec<&str> = code.split('\n').collect();
                if lines.last() == Some(&"") {
                    lines.pop();
                }
                let shared = lines
                    .iter()
                    .filter(|line| !line.trim_matches(is_space).is_empty())
                    .map(|line| line.len() - line.trim_start_matches(is_space).len())
                    .min()
                    .unwrap_or(0);
                for line in lines {
                    self.new_line(0);
                    self.current
                        .push_str(line.get(shared..).unwrap_or("").trim_end_matches(is_space));
                }
                self.new_line(0);
                self.current.push_str("}</pre>");
                self.request(Gap::BlankLine);
                self.last = Last::Block;
            }
            Token::ListOpen(tag) => {
                self.request(Gap::BlankLine);
                self.write(tag, indent);
                self.lists.push(ListLevel {
                    base: indent,
                    in_item: false,
                });
                self.request(Gap::Newline);
                self.last = Last::Block;
            }
            Token::ListClose(tag) => match self.lists.pop() {
                Some(level) => {
                    // The closing tag follows what the list holds directly.
                    self.gap = Gap::Newline;
                    self.write(tag, level.base);
                    self.request(Gap::BlankLine);
                    self.last = Last::Block;
                }
                None => {
                    self.write(tag, indent);
                    self.last = Last::Text;
                }
            },
            Token::ItemOpen(tag) => match self.lists.last_mut() {
                Some(level) => {
                    level.in_item = true;
                    let item_indent = level.base + ITEM_INDENT;
                    self.request(Gap::Newline);
                    self.write(tag, item_indent);
                    self.attach = true;
                    self.last = Last::Block;
                }
                None => {
                    self.write(tag, indent);
                    self.last = Last::Text;
                }
            },
            Token::HeadingOpen(tag) => {
                self.request(Gap::BlankLine);
                self.write(tag, indent);
                self.attach = true;
                self.last = Last::Block;
            }
            Token::HeadingClose(tag) => {
                self.gap = Gap::Soft;
                self.write(tag, indent);
                self.request(Gap::BlankLine);
                self.last = Last::Block;
            }
            Token::QuoteOpen(tag) => {
                self.request(Gap::BlankLine);
                self.write(tag, indent);
                self.request(Gap::Newline);
                self.last = Last::Block;
            }
            Token::QuoteClose(tag) => {
                self.request(Gap::Newline);
                self.write(tag, indent);
                self.request(Gap::BlankLine);
                self.last = Last::Block;
            }
            Token::LineBreak => {
                self.write("<br>", indent);
                self.request(Gap::Newline);
                self.last = Last::Text;
            }
            Token::HtmlComment(comment) => {
                self.request(Gap::Newline);
                self.write(comment, indent);
                self.request(Gap::Newline);
                self.last = Last::Text;
            }
        }
    }

    fn finish(&mut self) -> Vec<String> {
        if self.wrote_anything {
            let line = std::mem::take(&mut self.current);
            self.lines.push(line.trim_end_matches(is_space).to_string());
        }
        while self.lines.last().is_some_and(|line| line.is_empty()) {
            self.lines.pop();
        }
        std::mem::take(&mut self.lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(comment: &str) -> String {
        format(comment, 2, 100).join("\n")
    }

    #[test]
    fn short_comments_take_one_line() {
        assert_eq!(fmt("/**\n   * Short.\n   */"), "/** Short. */");
        assert_eq!(fmt("/** */"), "/** */");
        assert_eq!(fmt("/**\n   */"), "/** */");
        // A block tag always gets the long form.
        assert_eq!(
            fmt("/** @return the thing */"),
            "/**\n * @return the thing\n */"
        );
    }

    #[test]
    fn text_is_wrapped_and_paragraphs_are_marked() {
        assert_eq!(
            format("/** one two three\n *\n * four */", 0, 14).join("\n"),
            "/**\n * one two\n * three\n *\n * <p>four\n */"
        );
        assert_eq!(
            fmt("/**\n * A.\n * <p>\n * B.\n * </p>\n * @param x  the x\n * @return y\n */"),
            "/**\n * A.\n *\n * <p>B.\n *\n * @param x the x\n * @return y\n */"
        );
    }

    #[test]
    fn lists_are_indented_by_nesting() {
        assert_eq!(
            fmt("/**\n * L:\n * <ul><li>a</li><li>b<ol><li>c</ol></li></ul>\n * End.\n */"),
            "/**\n * L:\n *\n * <ul>\n *   <li>a\n *   <li>b\n *       <ol>\n *         <li>c\n *       </ol>\n * </ul>\n *\n * End.\n */"
        );
    }

    #[test]
    fn preformatted_text_is_kept() {
        assert_eq!(
            fmt("/**\n * A.\n * <pre>\n *   x   y\n * </pre>\n * B.\n */"),
            "/**\n * A.\n *\n * <pre>\n *   x   y\n * </pre>\n *\n * B.\n */"
        );
        assert_eq!(
            fmt("/**\n * <pre>{@code\n *     a();\n *   b();\n * }</pre>\n */"),
            "/**\n * <pre>{@code\n *   a();\n * b();\n * }</pre>\n */"
        );
    }
}
