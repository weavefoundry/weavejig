//! The layout document: an intermediate form between the syntax tree and
//! the printed text.
//!
//! Formatting rules describe *where* lines may break; the printer decides
//! which breaks to take so that lines fit the width. The model is the usual
//! one of groups that are printed flat if they fit and broken otherwise.

#[derive(Debug, Clone)]
pub(crate) enum Doc {
    Text(String),
    /// Text that takes no room when measuring whether a group fits. Used for
    /// trailing line comments, which never cause code to be re-wrapped.
    Suffix(String),
    /// A space, unless it would start a line.
    Space,
    /// A space when flat, a line break when broken.
    Line,
    /// Nothing when flat, a line break when broken.
    SoftLine,
    /// Always a line break. Several in a row still produce only one.
    HardLine,
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
}

/// Names a [`Doc::TaggedGroup`].
pub(crate) type Tag = u32;

impl Doc {
    /// Whether the document contains a mandatory line break, which forces
    /// every enclosing group to break.
    pub(crate) fn forces_break(&self) -> bool {
        match self {
            Doc::Text(text) => text.contains('\n'),
            Doc::Suffix(_) | Doc::Space | Doc::Line | Doc::SoftLine => false,
            Doc::HardLine | Doc::BlankLine => true,
            Doc::Concat(_, forced)
            | Doc::Indent(_, _, forced)
            | Doc::Group(_, forced)
            | Doc::TaggedGroup(_, _, forced)
            | Doc::IndentIf(_, _, _, forced)
            | Doc::GroupIf(_, _, forced)
            | Doc::FillBreak(_, forced) => *forced,
        }
    }
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

pub(crate) fn fill_break(doc: Doc) -> Doc {
    let forced = doc.forces_break();
    Doc::FillBreak(Box::new(doc), forced)
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
