//! Parser machinery: a token cursor that records tree-building events.
//!
//! The grammar (in [`crate::grammar`]) never touches trivia or the tree
//! directly. It looks at significant tokens and emits `Start`/`Token`/`Finish`
//! events through [`Marker`]s; [`build_tree`] then replays the events against
//! the full token list, attaching whitespace and comments as it goes.

use rowan::{GreenNode, GreenNodeBuilder};

use crate::{SyntaxError, SyntaxKind, SyntaxKind::*, TextRange, TextSize, Token, grammar};

/// Nesting depth at which the parser gives up rather than risk overflowing
/// the stack on pathological input.
const MAX_DEPTH: u32 = 400;

pub(crate) fn parse_tokens(text: &str, tokens: &[Token]) -> (GreenNode, Vec<SyntaxError>) {
    let mut parser = Parser::new(text, tokens);
    grammar::compilation_unit(&mut parser);
    let Parser { events, errors, .. } = parser;
    (build_tree(text, tokens, events), errors)
}

#[derive(Debug)]
enum Event {
    /// Starts a node. `kind` is `None` for abandoned markers. `forward_parent`
    /// points at a later `Start` event that should become this node's parent,
    /// which is how a node gets wrapped after it has been parsed.
    Start {
        kind: Option<SyntaxKind>,
        forward_parent: Option<usize>,
    },
    Finish,
    Token,
}

pub(crate) struct Parser<'t> {
    text: &'t str,
    /// Kinds of the significant (non-trivia) tokens.
    kinds: Vec<SyntaxKind>,
    ranges: Vec<TextRange>,
    pos: usize,
    events: Vec<Event>,
    errors: Vec<SyntaxError>,
    depth: u32,
    /// Set while parsing `case` labels, where `x -> ...` is not a lambda.
    pub(crate) no_lambda: bool,
}

impl<'t> Parser<'t> {
    fn new(text: &'t str, tokens: &[Token]) -> Parser<'t> {
        let mut kinds = Vec::new();
        let mut ranges = Vec::new();
        let mut offset = TextSize::new(0);
        for token in tokens {
            let range = TextRange::at(offset, token.len);
            offset = range.end();
            if !token.kind.is_trivia() {
                kinds.push(token.kind);
                ranges.push(range);
            }
        }
        Parser {
            text,
            kinds,
            ranges,
            pos: 0,
            events: Vec::new(),
            errors: Vec::new(),
            depth: 0,
            no_lambda: false,
        }
    }

    // --- Looking at tokens ---------------------------------------------

    /// Index of the current token, for use with the `*_at` lookahead methods.
    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// Kind of the token at absolute index `i`, or `EOF` past the end.
    pub(crate) fn kind_at(&self, i: usize) -> SyntaxKind {
        self.kinds.get(i).copied().unwrap_or(EOF)
    }

    pub(crate) fn text_at(&self, i: usize) -> &'t str {
        self.ranges.get(i).map_or("", |range| &self.text[*range])
    }

    /// Whether the token at `i` is the identifier `word`. Used for contextual
    /// keywords, which the lexer does not distinguish from identifiers.
    pub(crate) fn is_word_at(&self, i: usize, word: &str) -> bool {
        self.kind_at(i) == IDENT && self.text_at(i) == word
    }

    /// Whether the tokens at `i` and `i + 1` touch, with nothing in between.
    pub(crate) fn joint_at(&self, i: usize) -> bool {
        match (self.ranges.get(i), self.ranges.get(i + 1)) {
            (Some(a), Some(b)) => a.end() == b.start(),
            _ => false,
        }
    }

    pub(crate) fn current(&self) -> SyntaxKind {
        self.kind_at(self.pos)
    }

    pub(crate) fn nth(&self, n: usize) -> SyntaxKind {
        self.kind_at(self.pos + n)
    }

    pub(crate) fn at(&self, kind: SyntaxKind) -> bool {
        self.current() == kind
    }

    pub(crate) fn at_word(&self, word: &str) -> bool {
        self.is_word_at(self.pos, word)
    }

    pub(crate) fn at_end(&self) -> bool {
        self.pos >= self.kinds.len()
    }

    // --- Consuming tokens ----------------------------------------------

    /// Consumes the current token. Does nothing at the end of input.
    pub(crate) fn bump(&mut self) {
        if !self.at_end() {
            self.pos += 1;
            self.events.push(Event::Token);
        }
    }

    pub(crate) fn eat(&mut self, kind: SyntaxKind) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    pub(crate) fn expect(&mut self, kind: SyntaxKind) -> bool {
        if self.eat(kind) {
            return true;
        }
        let what = match kind {
            IDENT => "an identifier".to_string(),
            _ => format!("`{}`", kind.static_text().unwrap_or("?")),
        };
        // Something is missing, so point just past the previous token, where
        // it belongs, not at whatever happens to come next.
        let range = match self.pos.checked_sub(1).and_then(|i| self.ranges.get(i)) {
            Some(previous) => TextRange::empty(previous.end()),
            None => TextRange::empty(TextSize::new(0)),
        };
        self.error_at(range, &format!("expected {what}"));
        false
    }

    /// Reports an error at the current token.
    pub(crate) fn error(&mut self, message: &str) {
        let range = match self.ranges.get(self.pos) {
            Some(range) => *range,
            None => TextRange::empty(TextSize::new(self.text.len() as u32)),
        };
        self.error_at(range, message);
    }

    fn error_at(&mut self, range: TextRange, message: &str) {
        // One error per position is enough; the rest are usually fallout.
        if self.errors.last().is_some_and(|last| last.range == range) {
            return;
        }
        self.errors.push(SyntaxError {
            message: message.to_string(),
            range,
        });
    }

    /// Reports an error and wraps the current token in an `ERROR` node, so
    /// that the caller is guaranteed to make progress.
    pub(crate) fn err_and_bump(&mut self, message: &str) {
        self.error(message);
        if !self.at_end() {
            let m = self.start();
            self.bump();
            m.complete(self, ERROR);
        }
    }

    // --- Building nodes ------------------------------------------------

    pub(crate) fn start(&mut self) -> Marker {
        let pos = self.events.len();
        self.events.push(Event::Start {
            kind: None,
            forward_parent: None,
        });
        Marker { pos }
    }

    /// Guards recursive grammar rules. Returns `false`, having consumed the
    /// rest of the input as an error, when nesting is too deep to continue.
    pub(crate) fn enter(&mut self) -> bool {
        self.depth += 1;
        if self.depth <= MAX_DEPTH {
            return true;
        }
        if !self.at_end() {
            self.error("code is nested too deeply to parse");
            let m = self.start();
            while !self.at_end() {
                self.bump();
            }
            m.complete(self, ERROR);
        }
        false
    }

    pub(crate) fn leave(&mut self) {
        self.depth -= 1;
    }
}

/// A node that has been started but not yet given a kind.
pub(crate) struct Marker {
    pos: usize,
}

impl Marker {
    pub(crate) fn complete(self, p: &mut Parser<'_>, kind: SyntaxKind) -> CompletedMarker {
        match &mut p.events[self.pos] {
            Event::Start { kind: slot, .. } => *slot = Some(kind),
            _ => unreachable!("marker must point at a Start event"),
        }
        p.events.push(Event::Finish);
        CompletedMarker {
            pos: self.pos,
            kind,
        }
    }

    /// Drops the marker without creating a node.
    pub(crate) fn abandon(self, p: &mut Parser<'_>) {
        if self.pos == p.events.len() - 1 {
            p.events.pop();
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct CompletedMarker {
    pos: usize,
    kind: SyntaxKind,
}

impl CompletedMarker {
    /// Starts a new node that will wrap this completed one.
    pub(crate) fn precede(self, p: &mut Parser<'_>) -> Marker {
        let m = p.start();
        match &mut p.events[self.pos] {
            Event::Start { forward_parent, .. } => *forward_parent = Some(m.pos),
            _ => unreachable!("marker must point at a Start event"),
        }
        m
    }

    pub(crate) fn kind(self) -> SyntaxKind {
        self.kind
    }
}

fn build_tree(text: &str, tokens: &[Token], mut events: Vec<Event>) -> GreenNode {
    let mut builder = GreenNodeBuilder::new();
    let mut next = 0; // index into `tokens`
    let mut offset = 0usize;
    let mut depth = 0u32;
    let mut parents = Vec::new();

    let mut push_token = |builder: &mut GreenNodeBuilder<'_>, next: &mut usize| {
        let token = tokens[*next];
        let len = u32::from(token.len) as usize;
        builder.token(token.kind.into(), &text[offset..offset + len]);
        offset += len;
        *next += 1;
    };

    for i in 0..events.len() {
        match std::mem::replace(
            &mut events[i],
            Event::Start {
                kind: None,
                forward_parent: None,
            },
        ) {
            Event::Start {
                kind,
                forward_parent,
            } => {
                // Collect this node and the chain of nodes that wrap it,
                // innermost first.
                parents.clear();
                parents.extend(kind);
                let mut forward = forward_parent;
                while let Some(index) = forward {
                    match std::mem::replace(
                        &mut events[index],
                        Event::Start {
                            kind: None,
                            forward_parent: None,
                        },
                    ) {
                        Event::Start {
                            kind,
                            forward_parent,
                        } => {
                            parents.extend(kind);
                            forward = forward_parent;
                        }
                        _ => unreachable!("forward_parent must point at a Start event"),
                    }
                }
                if parents.is_empty() {
                    continue;
                }
                // Trivia before a node belongs to its parent, so that every
                // node starts at a significant token. The root takes it all.
                if depth > 0 {
                    while next < tokens.len() && tokens[next].kind.is_trivia() {
                        push_token(&mut builder, &mut next);
                    }
                }
                for kind in parents.drain(..).rev() {
                    builder.start_node(kind.into());
                    depth += 1;
                }
            }
            Event::Token => {
                while tokens[next].kind.is_trivia() {
                    push_token(&mut builder, &mut next);
                }
                push_token(&mut builder, &mut next);
            }
            Event::Finish => {
                if depth == 1 {
                    while next < tokens.len() {
                        push_token(&mut builder, &mut next);
                    }
                }
                builder.finish_node();
                depth -= 1;
            }
        }
    }
    builder.finish()
}
