//! Java lexer. Lossless: the concatenated token texts reproduce the input.

use crate::{SyntaxError, SyntaxKind, SyntaxKind::*, TextRange, TextSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: SyntaxKind,
    pub len: TextSize,
}

/// Splits `text` into tokens, including whitespace and comments.
pub fn lex(text: &str) -> (Vec<Token>, Vec<SyntaxError>) {
    let mut lexer = Lexer {
        text,
        pos: 0,
        errors: Vec::new(),
    };
    let mut tokens = Vec::new();
    while lexer.pos < text.len() {
        let start = lexer.pos;
        let kind = lexer.next_kind();
        debug_assert!(lexer.pos > start, "lexer must make progress");
        tokens.push(Token {
            kind,
            len: TextSize::new((lexer.pos - start) as u32),
        });
    }
    (tokens, lexer.errors)
}

struct Lexer<'a> {
    text: &'a str,
    pos: usize,
    errors: Vec<SyntaxError>,
}

fn is_ident_start(c: char) -> bool {
    c == '_' || c == '$' || c.is_alphabetic()
}

fn is_ident_part(c: char) -> bool {
    c == '_' || c == '$' || c.is_alphanumeric()
}

impl Lexer<'_> {
    fn peek(&self) -> Option<char> {
        self.text[self.pos..].chars().next()
    }

    fn at(&self, s: &str) -> bool {
        self.text[self.pos..].starts_with(s)
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.pos += c.len_utf8();
            true
        } else {
            false
        }
    }

    fn eat_while(&mut self, pred: impl Fn(char) -> bool) -> usize {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if !pred(c) {
                break;
            }
            self.pos += c.len_utf8();
        }
        self.pos - start
    }

    fn error(&mut self, start: usize, message: &str) {
        let range = TextRange::new(TextSize::new(start as u32), TextSize::new(self.pos as u32));
        self.errors.push(SyntaxError {
            message: message.to_string(),
            range,
        });
    }

    fn next_kind(&mut self) -> SyntaxKind {
        let start = self.pos;
        let c = self.bump().expect("caller checked for end of input");
        match c {
            ' ' | '\t' | '\n' | '\r' | '\x0C' => {
                self.eat_while(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0C'));
                WHITESPACE
            }
            '/' if self.eat('/') => {
                self.eat_while(|c| c != '\n' && c != '\r');
                LINE_COMMENT
            }
            '/' if self.eat('*') => {
                match self.text[self.pos..].find("*/") {
                    Some(end) => self.pos += end + 2,
                    None => {
                        self.pos = self.text.len();
                        self.error(start, "unterminated comment");
                    }
                }
                BLOCK_COMMENT
            }
            '"' => {
                if self.at("\"\"") {
                    self.pos += 2;
                    self.text_block(start)
                } else {
                    self.quoted(start, '"', "unterminated string literal");
                    STRING_LITERAL
                }
            }
            '\'' => {
                if self.peek() == Some('\'') {
                    self.pos += 1;
                    self.error(start, "empty character literal");
                } else {
                    self.quoted(start, '\'', "unterminated character literal");
                }
                CHAR_LITERAL
            }
            '0'..='9' => self.number(start, c),
            '.' if self.peek().is_some_and(|c| c.is_ascii_digit()) => self.number(start, c),
            c if is_ident_start(c) => {
                self.eat_while(is_ident_part);
                SyntaxKind::from_keyword(&self.text[start..self.pos]).unwrap_or(IDENT)
            }
            _ => self.punctuation(start, c),
        }
    }

    /// Lexes the rest of a string or character literal after its opening quote.
    fn quoted(&mut self, start: usize, quote: char, unterminated: &str) {
        loop {
            match self.peek() {
                None | Some('\n' | '\r') => {
                    self.error(start, unterminated);
                    return;
                }
                Some('\\') => {
                    self.pos += 1;
                    if !matches!(self.peek(), None | Some('\n' | '\r')) {
                        self.bump();
                    }
                }
                Some(c) => {
                    self.pos += c.len_utf8();
                    if c == quote {
                        return;
                    }
                }
            }
        }
    }

    /// Lexes the rest of a text block after its opening `"""`.
    fn text_block(&mut self, start: usize) -> SyntaxKind {
        loop {
            if self.at("\"\"\"") {
                self.pos += 3;
                return TEXT_BLOCK;
            }
            match self.bump() {
                None => {
                    self.error(start, "unterminated text block");
                    return TEXT_BLOCK;
                }
                Some('\\') => {
                    self.bump();
                }
                Some(_) => {}
            }
        }
    }

    fn number(&mut self, start: usize, first: char) -> SyntaxKind {
        let digits = |c: char| c.is_ascii_digit() || c == '_';
        let hex_digits = |c: char| c.is_ascii_hexdigit() || c == '_';

        if first == '0' && matches!(self.peek(), Some('x' | 'X')) {
            self.pos += 1;
            let mut any = self.eat_while(hex_digits) > 0;
            let mut float = false;
            if self.peek() == Some('.') {
                self.pos += 1;
                any |= self.eat_while(hex_digits) > 0;
                float = true;
            }
            if matches!(self.peek(), Some('p' | 'P')) {
                self.pos += 1;
                if matches!(self.peek(), Some('+' | '-')) {
                    self.pos += 1;
                }
                if self.eat_while(digits) == 0 {
                    self.error(start, "malformed floating-point literal");
                }
                if matches!(self.peek(), Some('f' | 'F' | 'd' | 'D')) {
                    self.pos += 1;
                }
                if !any {
                    self.error(start, "hexadecimal numbers must contain at least one digit");
                }
                return FLOAT_LITERAL;
            }
            if float {
                self.error(start, "malformed floating-point literal");
                return FLOAT_LITERAL;
            }
            if !any {
                self.error(start, "hexadecimal numbers must contain at least one digit");
            }
            if matches!(self.peek(), Some('l' | 'L')) {
                self.pos += 1;
            }
            return INT_LITERAL;
        }

        if first == '0' && matches!(self.peek(), Some('b' | 'B')) {
            self.pos += 1;
            if self.eat_while(digits) == 0 {
                self.error(start, "binary numbers must contain at least one digit");
            }
            if matches!(self.peek(), Some('l' | 'L')) {
                self.pos += 1;
            }
            return INT_LITERAL;
        }

        let mut float = first == '.';
        self.eat_while(digits);
        if !float && self.peek() == Some('.') {
            self.pos += 1;
            self.eat_while(digits);
            float = true;
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some('+' | '-')) {
                self.pos += 1;
            }
            if self.eat_while(digits) == 0 {
                self.error(start, "malformed floating-point literal");
            }
            float = true;
        }
        if matches!(self.peek(), Some('f' | 'F' | 'd' | 'D')) {
            self.pos += 1;
            float = true;
        }
        if float {
            return FLOAT_LITERAL;
        }
        if matches!(self.peek(), Some('l' | 'L')) {
            self.pos += 1;
        }
        INT_LITERAL
    }

    fn punctuation(&mut self, start: usize, first: char) -> SyntaxKind {
        // `first` is already consumed; `rest` lists the continuations to try,
        // longest first.
        let mut pick = |rest: &[(&str, SyntaxKind)], otherwise: SyntaxKind| {
            for (suffix, kind) in rest {
                if self.at(suffix) {
                    self.pos += suffix.len();
                    return *kind;
                }
            }
            otherwise
        };
        match first {
            '(' => L_PAREN,
            ')' => R_PAREN,
            '{' => L_BRACE,
            '}' => R_BRACE,
            '[' => L_BRACK,
            ']' => R_BRACK,
            ';' => SEMI,
            ',' => COMMA,
            '@' => AT,
            '~' => TILDE,
            '?' => QUESTION,
            '.' => pick(&[("..", ELLIPSIS)], DOT),
            ':' => pick(&[(":", COLON_COLON)], COLON),
            '=' => pick(&[("=", EQ_EQ)], EQ),
            '!' => pick(&[("=", BANG_EQ)], BANG),
            '<' => pick(&[("<=", LT_LT_EQ), ("<", LT_LT), ("=", LT_EQ)], LT),
            '>' => pick(&[(">>=", GT_GT_GT_EQ), (">=", GT_GT_EQ), ("=", GT_EQ)], GT),
            '+' => pick(&[("+", PLUS_PLUS), ("=", PLUS_EQ)], PLUS),
            '-' => pick(&[("-", MINUS_MINUS), ("=", MINUS_EQ), (">", ARROW)], MINUS),
            '*' => pick(&[("=", STAR_EQ)], STAR),
            '/' => pick(&[("=", SLASH_EQ)], SLASH),
            '&' => pick(&[("&", AMP_AMP), ("=", AMP_EQ)], AMP),
            '|' => pick(&[("|", PIPE_PIPE), ("=", PIPE_EQ)], PIPE),
            '^' => pick(&[("=", CARET_EQ)], CARET),
            '%' => pick(&[("=", PERCENT_EQ)], PERCENT),
            // A trailing Ctrl-Z is permitted (and ignored) at the end of input.
            '\x1A' if self.pos == self.text.len() => WHITESPACE,
            _ => {
                self.error(
                    start,
                    &format!("illegal character: '{}'", first.escape_default()),
                );
                ERROR_TOKEN
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<SyntaxKind> {
        let (tokens, errors) = lex(text);
        assert!(
            errors.is_empty(),
            "unexpected errors for {text:?}: {errors:?}"
        );
        let total: u32 = tokens.iter().map(|t| u32::from(t.len)).sum();
        assert_eq!(total as usize, text.len());
        tokens
            .into_iter()
            .map(|t| t.kind)
            .filter(|k| !k.is_trivia())
            .collect()
    }

    #[test]
    fn words() {
        assert_eq!(
            kinds("class Foo $x _y var"),
            [CLASS_KW, IDENT, IDENT, IDENT, IDENT]
        );
        assert_eq!(kinds("true false null"), [TRUE_KW, FALSE_KW, NULL_KW]);
    }

    #[test]
    fn numbers() {
        for int in ["0", "42", "1_000", "0x1F", "0b1010", "017", "42L", "0xFFl"] {
            assert_eq!(kinds(int), [INT_LITERAL], "{int}");
        }
        for float in [
            "1.0", "1.", ".5", "1e10", "1.5e-3f", "2d", "0x1.8p3", "0x1p-2f", "1_0.0_1",
        ] {
            assert_eq!(kinds(float), [FLOAT_LITERAL], "{float}");
        }
        assert_eq!(kinds("x.5"), [IDENT, FLOAT_LITERAL]);
        assert_eq!(kinds("a...b"), [IDENT, ELLIPSIS, IDENT]);
    }

    #[test]
    fn strings_and_chars() {
        assert_eq!(
            kinds(r#""a\"b" 'c' '\'' '\\' 'A'"#),
            [
                STRING_LITERAL,
                CHAR_LITERAL,
                CHAR_LITERAL,
                CHAR_LITERAL,
                CHAR_LITERAL
            ]
        );
        assert_eq!(
            kinds("\"\" + \"\"\"\n  hi \\\"\"\" there\n  \"\"\""),
            [STRING_LITERAL, PLUS, TEXT_BLOCK]
        );
    }

    #[test]
    fn comments() {
        let (tokens, errors) = lex("a // one\n/* two\n */ b /** doc */");
        assert!(errors.is_empty());
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            [
                IDENT,
                WHITESPACE,
                LINE_COMMENT,
                WHITESPACE,
                BLOCK_COMMENT,
                WHITESPACE,
                IDENT,
                WHITESPACE,
                BLOCK_COMMENT
            ]
        );
    }

    #[test]
    fn angle_brackets() {
        // Shifts are left as separate `>` tokens for the parser to join.
        assert_eq!(
            kinds("a >> b >>> c"),
            [IDENT, GT, GT, IDENT, GT, GT, GT, IDENT]
        );
        assert_eq!(
            kinds("a >>= b >>>= c >= d"),
            [IDENT, GT_GT_EQ, IDENT, GT_GT_GT_EQ, IDENT, GT_EQ, IDENT]
        );
        assert_eq!(
            kinds("a << b <<= c <= d"),
            [IDENT, LT_LT, IDENT, LT_LT_EQ, IDENT, LT_EQ, IDENT]
        );
    }

    #[test]
    fn operators() {
        assert_eq!(
            kinds("-> :: ++ -- && || != =="),
            [
                ARROW,
                COLON_COLON,
                PLUS_PLUS,
                MINUS_MINUS,
                AMP_AMP,
                PIPE_PIPE,
                BANG_EQ,
                EQ_EQ
            ]
        );
    }

    #[test]
    fn errors() {
        for bad in ["\"abc", "'a", "/* abc", "#", "\"\"\"abc", "0x", "''"] {
            let (tokens, errors) = lex(bad);
            assert!(!errors.is_empty(), "{bad}");
            let total: u32 = tokens.iter().map(|t| u32::from(t.len)).sum();
            assert_eq!(total as usize, bad.len());
        }
    }
}
