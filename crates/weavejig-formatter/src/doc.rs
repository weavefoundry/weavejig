//! The layout document: an intermediate form between the syntax tree and
//! the printed text.
//!
//! Formatting rules describe *where* lines may break; the printer decides
//! which breaks to take so that lines fit the width. The model is the usual
//! one of groups that are printed flat if they fit and broken otherwise.

#[derive(Debug, Clone)]
pub(crate) enum Doc {
    Text(String),
    /// A trailing line comment. It counts when measuring whether a group
    /// fits: code is wrapped to make room for the comment after it. One
    /// that still runs past the line width is wrapped onto `//` lines that
    /// start in its column.
    Suffix(String),
    /// A space, unless it would start a line.
    Space,
    /// A space when flat, a line break when broken.
    Line,
    /// Nothing when flat, a line break when broken.
    SoftLine,
    /// Always a line break. Several in a row still produce only one.
    HardLine,
    /// The line break that ends a trailing `//` comment. It forces nothing
    /// to break where it is written: [`place_comment_breaks`] moves it out
    /// of every document that it ends, and only what still encloses it
    /// then is broken.
    CommentBreak,
    /// Always a line break, preceded by exactly one empty line.
    BlankLine,
    /// The `bool` on the next three caches [`Doc::forces_break`].
    Concat(Vec<Doc>, bool),
    /// Indents line breaks inside the document by a number of columns.
    Indent(u16, Box<Doc>, bool),
    /// Printed flat if it fits on the line, otherwise broken: every direct
    /// `Line` and `SoftLine` becomes a line break.
    Group(Box<Doc>, bool),
    /// A group whose outcome [`Doc::IndentIf`] can refer to.
    TaggedGroup(Tag, Box<Doc>, bool),
    /// Indents like [`Doc::Indent`], but only if the group with this tag,
    /// which must be printed earlier, was broken. This is how the arguments
    /// of `a.b(...)` move further right once the chain after it wraps.
    IndentIf(Tag, u16, Box<Doc>, bool),
    /// A group that is also broken whenever the group with this tag, which
    /// must be printed earlier, was broken.
    GroupIf(Tag, Box<Doc>, bool),
    /// A line break followed by a document. When flat, a space. Otherwise
    /// the break is taken only if the document, printed flat, does not fit
    /// on the current line. A run of these fills lines with as many items
    /// as fit, as in a list of numbers.
    FillBreak(Box<Doc>, bool),
    /// A document that may start on the next line, indented by a number of
    /// columns. It stays where it is if it fits there flat; failing that it
    /// moves to the next line if it fits there flat; failing that it stays
    /// and is broken. This is how a lambda that is a call's only argument
    /// is laid out.
    NextLineIfFits(u16, Box<Doc>, bool),
    /// A `//` comment on a line of its own, which is wrapped at the line
    /// width.
    LineComment(String),
    /// A `/** ... */` comment, which is reflowed for the column it lands on.
    Javadoc(String),
    /// A text block, whose lines are indented like the line that its
    /// opening delimiter lands on.
    TextBlock(String),
}

/// Names a [`Doc::TaggedGroup`].
pub(crate) type Tag = u32;

impl Doc {
    /// Whether the document contains a mandatory line break, which forces
    /// every enclosing group to break.
    pub(crate) fn forces_break(&self) -> bool {
        match self {
            Doc::Text(text) => text.contains('\n'),
            Doc::Suffix(_) | Doc::Space | Doc::Line | Doc::SoftLine | Doc::CommentBreak => false,
            Doc::HardLine | Doc::BlankLine => true,
            Doc::Concat(_, forced)
            | Doc::Indent(_, _, forced)
            | Doc::Group(_, forced)
            | Doc::TaggedGroup(_, _, forced)
            | Doc::IndentIf(_, _, _, forced)
            | Doc::GroupIf(_, _, forced)
            | Doc::FillBreak(_, forced)
            | Doc::NextLineIfFits(_, _, forced) => *forced,
            Doc::LineComment(_) => false,
            Doc::Javadoc(text) => text.contains('\n'),
            Doc::TextBlock(_) => true,
        }
    }
}

impl Doc {
    /// The width of the document printed on one line, ignoring any
    /// mandatory line breaks in it.
    pub(crate) fn flat_width(&self) -> usize {
        match self {
            Doc::Text(text) | Doc::Javadoc(text) => text.chars().count(),
            Doc::Suffix(text) | Doc::LineComment(text) => comment_width(text),
            // Only the opening delimiter is on the line.
            Doc::TextBlock(_) => 3,
            Doc::Space | Doc::Line => 1,
            Doc::SoftLine | Doc::HardLine | Doc::BlankLine | Doc::CommentBreak => 0,
            Doc::Concat(docs, _) => docs.iter().map(Doc::flat_width).sum(),
            Doc::Indent(_, doc, _)
            | Doc::Group(doc, _)
            | Doc::TaggedGroup(_, doc, _)
            | Doc::IndentIf(_, _, doc, _)
            | Doc::GroupIf(_, doc, _)
            | Doc::NextLineIfFits(_, doc, _) => doc.flat_width(),
            Doc::FillBreak(doc, _) => 1 + doc.flat_width(),
        }
    }
}

/// The columns a `//` comment takes when it is decided what fits on a
/// line. An empty one counts as `// `, one more than it prints as:
/// google-java-format measures it so.
pub(crate) fn comment_width(comment: &str) -> usize {
    comment.chars().count() + usize::from(comment == "//")
}

pub(crate) fn text(text: impl Into<String>) -> Doc {
    Doc::Text(text.into())
}

pub(crate) fn space() -> Doc {
    Doc::Space
}

pub(crate) fn concat(docs: Vec<Doc>) -> Doc {
    let forced = docs.iter().any(Doc::forces_break);
    Doc::Concat(docs, forced)
}

pub(crate) fn indent(columns: u16, doc: Doc) -> Doc {
    let forced = doc.forces_break();
    Doc::Indent(columns, Box::new(doc), forced)
}

pub(crate) fn group(doc: Doc) -> Doc {
    let forced = doc.forces_break();
    Doc::Group(Box::new(doc), forced)
}

pub(crate) fn tagged_group(tag: Tag, doc: Doc) -> Doc {
    let forced = doc.forces_break();
    Doc::TaggedGroup(tag, Box::new(doc), forced)
}

pub(crate) fn indent_if(tag: Tag, columns: u16, doc: Doc) -> Doc {
    let forced = doc.forces_break();
    Doc::IndentIf(tag, columns, Box::new(doc), forced)
}

pub(crate) fn group_if(tag: Tag, doc: Doc) -> Doc {
    let forced = doc.forces_break();
    Doc::GroupIf(tag, Box::new(doc), forced)
}

pub(crate) fn next_line_if_fits(columns: u16, doc: Doc) -> Doc {
    let forced = doc.forces_break();
    Doc::NextLineIfFits(columns, Box::new(doc), forced)
}

pub(crate) fn fill_break(doc: Doc) -> Doc {
    let forced = doc.forces_break();
    Doc::FillBreak(Box::new(doc), forced)
}

/// Turns every [`Doc::CommentBreak`] into a [`Doc::HardLine`], placed after
/// the documents it ends: a comment that trails the last token of a group
/// breaks the line after the group, so the group itself may stay flat.
///
/// google-java-format does the same: `a.b().c(x) // why` followed by
/// `&& d` breaks before the `&&`, but not the chain, whose last token the
/// comment trails. The `HardLine` lands where the document that follows the
/// comment starts, and forces what encloses it there to break.
pub(crate) fn place_comment_breaks(mut doc: Doc) -> Doc {
    if hoist_comment_break(&mut doc) {
        doc = concat(vec![doc, Doc::HardLine]);
    }
    doc
}

/// Places the comment breaks inside `doc`, except one that ends it: that one
/// is removed and reported, for the caller to place after `doc`. Updates
/// the cached [`Doc::forces_break`] of everything it changes.
fn hoist_comment_break(doc: &mut Doc) -> bool {
    match doc {
        Doc::CommentBreak => {
            *doc = Doc::Concat(Vec::new(), false);
            true
        }
        Doc::Concat(docs, forced) => {
            let mut pending = false;
            let mut i = 0;
            while i < docs.len() {
                let ends = hoist_comment_break(&mut docs[i]);
                if pending && !is_empty(&docs[i]) {
                    docs.insert(i, Doc::HardLine);
                    i += 1;
                    pending = false;
                }
                pending |= ends;
                i += 1;
            }
            *forced = docs.iter().any(Doc::forces_break);
            pending
        }
        Doc::Indent(_, inner, forced)
        | Doc::Group(inner, forced)
        | Doc::TaggedGroup(_, inner, forced)
        | Doc::IndentIf(_, _, inner, forced)
        | Doc::GroupIf(_, inner, forced)
        | Doc::FillBreak(inner, forced)
        | Doc::NextLineIfFits(_, inner, forced) => {
            let ends = hoist_comment_break(inner);
            *forced = inner.forces_break();
            ends
        }
        Doc::Text(_)
        | Doc::Suffix(_)
        | Doc::Space
        | Doc::Line
        | Doc::SoftLine
        | Doc::HardLine
        | Doc::BlankLine
        | Doc::LineComment(_)
        | Doc::Javadoc(_)
        | Doc::TextBlock(_) => false,
    }
}

/// Whether `doc` prints nothing at all, not even a space.
fn is_empty(doc: &Doc) -> bool {
    match doc {
        Doc::Text(text) => text.is_empty(),
        Doc::Concat(docs, _) => docs.iter().all(is_empty),
        Doc::Indent(_, inner, _)
        | Doc::Group(inner, _)
        | Doc::TaggedGroup(_, inner, _)
        | Doc::IndentIf(_, _, inner, _)
        | Doc::GroupIf(_, inner, _) => is_empty(inner),
        _ => false,
    }
}

impl Doc {
    /// Whether the document contains the line break after a trailing `//`
    /// comment that [`place_comment_breaks`] has not placed yet.
    pub(crate) fn has_comment_break(&self) -> bool {
        match self {
            Doc::CommentBreak => true,
            Doc::Concat(docs, _) => docs.iter().any(Doc::has_comment_break),
            Doc::Indent(_, inner, _)
            | Doc::Group(inner, _)
            | Doc::TaggedGroup(_, inner, _)
            | Doc::IndentIf(_, _, inner, _)
            | Doc::GroupIf(_, inner, _)
            | Doc::FillBreak(inner, _)
            | Doc::NextLineIfFits(_, inner, _) => inner.has_comment_break(),
            _ => false,
        }
    }
}

/// `items` separated by `separator` followed by a `Line`.
#[cfg(test)]
pub(crate) fn join_lines(items: Vec<Doc>, separator: &str) -> Doc {
    let mut docs = Vec::with_capacity(items.len() * 3);
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            docs.push(text(separator));
            docs.push(Doc::Line);
        }
        docs.push(item);
    }
    concat(docs)
}
