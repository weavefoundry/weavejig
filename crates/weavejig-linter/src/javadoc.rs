//! Reading Javadoc comments, as far as rules about code need to.

/// Whether a block comment is a Javadoc comment. `/**/` is an empty
/// ordinary comment.
pub(crate) fn is_javadoc(comment: &str) -> bool {
    comment.starts_with("/**") && comment.len() > 4
}

/// The lines of a Javadoc comment without the comment markers and the
/// leading asterisks.
pub(crate) fn lines(comment: &str) -> Vec<&str> {
    let body = comment
        .strip_prefix("/**")
        .unwrap_or(comment)
        .strip_suffix("*/")
        .unwrap_or(comment);
    body.lines()
        .map(|line| {
            let line = line.trim();
            line.strip_prefix('*').map_or(line, str::trim_start)
        })
        .collect()
}

/// Whether the comment has the block tag `tag` (written with its `@`), at
/// the start of a line.
pub(crate) fn has_block_tag(comment: &str, tag: &str) -> bool {
    lines(comment).iter().any(|line| {
        line.strip_prefix(tag)
            .is_some_and(|rest| !rest.starts_with(|c: char| c.is_alphanumeric()))
    })
}

/// Whether the comment has the inline tag `{@name ...}` anywhere.
pub(crate) fn has_inline_tag(comment: &str, name: &str) -> bool {
    let mut rest = comment;
    while let Some(at) = rest.find("{@") {
        rest = &rest[at + 2..];
        if let Some(after) = rest.strip_prefix(name)
            && !after.starts_with(|c: char| c.is_alphanumeric())
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags() {
        let doc =
            "/**\n * Text {@inheritDoc} more.\n *\n * @deprecated use that\n * @param x y\n */";
        assert!(is_javadoc(doc));
        assert!(!is_javadoc("/**/"));
        assert!(has_block_tag(doc, "@deprecated"));
        assert!(has_block_tag(doc, "@param"));
        assert!(!has_block_tag(doc, "@dep"));
        assert!(!has_block_tag(doc, "@return"));
        assert!(has_inline_tag(doc, "inheritDoc"));
        assert!(!has_inline_tag(doc, "inherit"));
        assert!(!has_inline_tag("/** inheritDoc */", "inheritDoc"));
    }
}
