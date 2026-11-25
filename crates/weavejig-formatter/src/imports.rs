//! Unused imports, and the order of the import section.
//!
//! Before it formats a file, google-java-format removes the imports nothing
//! uses and then sorts the rest. [`fix`] does both to the source text, and
//! the formatter lays the result out. The rules, as observed:
//!
//! - A single-type or static import is used when its simple name is: when
//!   an identifier in the code has that name ([`code_names`]), or a Javadoc
//!   reference on a declaration starts with it (`{@link Name}`,
//!   `@see Name#member(Name)`, and so on). Javadoc elsewhere, ordinary
//!   comments, `{@code Name}`, member names (`x.Name`) and the names
//!   declarations introduce do not count.
//! - Wildcard imports always stay. Non-static imports from `java.lang` or
//!   from the file's own package always go, used or not.
//! - Each import carries the comments that follow it: a `//` comment on its
//!   line, then `//` comments that start in the first column of each line
//!   directly below it. Sorting moves them with the import. Comments before
//!   the first import stay at the top.
//! - Anything else ends the import section: a blank line or another kind
//!   of comment, followed by more imports, is refused by google-java-format.
//!   For such a file, unused imports are removed here and the formatter
//!   sorts what is left in its own way.
//! - The comments of a removed import join those of the import before it,
//!   or the comments at the top when there is none.

use std::collections::HashSet;

use weavejig_syntax::{
    SyntaxKind::{self, *},
    SyntaxNode, SyntaxToken, TextRange, TextSize, child_node, first_token, last_token,
    prev_significant_token, significant_children,
};

use crate::Edit;

/// Nodes whose own identifiers are names they declare, not references.
const DECLARES_NAMES: [SyntaxKind; 15] = [
    CLASS_DECL,
    INTERFACE_DECL,
    ENUM_DECL,
    RECORD_DECL,
    ANNOTATION_TYPE_DECL,
    METHOD_DECL,
    CONSTRUCTOR_DECL,
    VARIABLE_DECLARATOR,
    PARAM,
    TYPE_PARAM,
    ENUM_CONSTANT,
    TYPE_PATTERN,
    LABELED_STMT,
    BREAK_STMT,
    CONTINUE_STMT,
];

/// Declarations whose Javadoc google-java-format reads for references.
const DOCUMENTED: [SyntaxKind; 11] = [
    PACKAGE_DECL,
    MODULE_DECL,
    CLASS_DECL,
    INTERFACE_DECL,
    ENUM_DECL,
    RECORD_DECL,
    ANNOTATION_TYPE_DECL,
    METHOD_DECL,
    CONSTRUCTOR_DECL,
    FIELD_DECL,
    ENUM_CONSTANT,
];

/// The dotted name an import declaration names, and whether it is static.
pub(crate) fn import_name(import: &SyntaxNode) -> (bool, String) {
    let mut is_static = false;
    let mut name = String::new();
    for element in significant_children(import) {
        match element.kind() {
            IMPORT_KW | SEMI => {}
            STATIC_KW => is_static = true,
            _ => name.push_str(&element.to_string()),
        }
    }
    (is_static, name.split_whitespace().collect())
}

/// The package the file declares.
pub(crate) fn package_name(root: &SyntaxNode) -> Option<String> {
    let name = child_node(&child_node(root, PACKAGE_DECL)?, NAME)?;
    Some(
        name.descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|token| !token.kind().is_trivia())
            .map(|token| token.text().to_string())
            .collect(),
    )
}

/// Whether an import does nothing: it imports a class, or every class, from
/// `java.lang` or from the file's own package, which are in scope anyway.
pub(crate) fn is_redundant(is_static: bool, name: &str, package: Option<&str>) -> bool {
    match name.rsplit_once('.') {
        Some((qualifier, _)) if !is_static => {
            qualifier == "java.lang" || Some(qualifier) == package
        }
        _ => false,
    }
}

/// The simple names the code refers to: every identifier outside the
/// package and import declarations, except a member's name (after `.` or
/// `::`) and a name a declaration introduces.
pub(crate) fn code_names(root: &SyntaxNode) -> HashSet<String> {
    let mut names = HashSet::new();
    for token in root
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == IDENT)
    {
        let Some(parent) = token.parent() else {
            continue;
        };
        let in_header = parent.ancestors().any(|node| {
            node.kind() == IMPORT_DECL
                || (node.kind() == NAME && node.parent().is_some_and(|p| p.kind() == PACKAGE_DECL))
        });
        let is_member = prev_significant_token(&token)
            .is_some_and(|prev| matches!(prev.kind(), DOT | COLON_COLON));
        if !in_header && !is_member && !DECLARES_NAMES.contains(&parent.kind()) {
            names.insert(token.text().to_string());
        }
    }
    names
}

fn is_javadoc(comment: &str) -> bool {
    comment.starts_with("/**") && comment.len() > 4
}

/// Adds the names that Javadoc on declarations refers to.
fn javadoc_names(root: &SyntaxNode, names: &mut HashSet<String>) {
    for node in root
        .descendants()
        .filter(|node| DOCUMENTED.contains(&node.kind()))
    {
        let mut token = first_token(&node).and_then(|first| first.prev_token());
        while let Some(trivia) = token.filter(|token| token.kind().is_trivia()) {
            if trivia.kind() == BLOCK_COMMENT && is_javadoc(trivia.text()) {
                references(trivia.text(), names);
            }
            token = trivia.prev_token();
        }
    }
}

/// Adds the names a Javadoc comment's references start with: those of
/// `{@link}`, `{@linkplain}`, `{@value}`, `@see`, `@throws` and
/// `@exception`, and the parameter types of a referenced method.
fn references(comment: &str, names: &mut HashSet<String>) {
    let body = comment
        .strip_prefix("/**")
        .and_then(|body| body.strip_suffix("*/"))
        .unwrap_or(comment);
    let lines: Vec<&str> = body
        .lines()
        .map(|line| {
            let line = line.trim_start();
            line.strip_prefix('*').unwrap_or(line).trim_start()
        })
        .collect();
    for line in &lines {
        for tag in ["@see", "@throws", "@exception"] {
            if let Some(rest) = line.strip_prefix(tag)
                && rest.starts_with(char::is_whitespace)
            {
                reference(rest, names);
            }
        }
    }
    let text = lines.join("\n");
    let mut rest = text.as_str();
    while let Some(at) = rest.find("{@") {
        rest = &rest[at + 2..];
        let tag_end = rest
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(rest.len());
        let (tag, after) = rest.split_at(tag_end);
        if matches!(tag, "link" | "linkplain" | "value") && after.starts_with(char::is_whitespace) {
            reference(after, names);
        }
    }
}

/// Adds the names a Javadoc reference such as `Foo.Bar#baz(Qux, int)` at
/// the start of `text` begins with.
fn reference(text: &str, names: &mut HashSet<String>) {
    let text = text.trim_start();
    let end = text
        .find(|c: char| c.is_whitespace() || c == '}' || c == '(')
        .unwrap_or(text.len());
    names.extend(leading_identifier(&text[..end]));
    if let Some(params) = text[end..].strip_prefix('(') {
        let params = &params[..params.find(')').unwrap_or(params.len())];
        for param in params.split(',') {
            names.extend(leading_identifier(param.trim_start()));
        }
    }
}

fn leading_identifier(text: &str) -> Option<String> {
    let end = text
        .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
        .unwrap_or(text.len());
    let name = &text[..end];
    (!name.is_empty() && !name.starts_with(|c: char| c.is_ascii_digit())).then(|| name.to_string())
}

/// An import declaration and the comments that travel with it.
struct Entry {
    is_static: bool,
    name: String,
    /// The declaration as written, `import` to `;`.
    range: TextRange,
    /// Comments, and whether each is on the import's own line.
    comments: Vec<(bool, String)>,
    keep: bool,
}

fn line_breaks(whitespace: &str) -> usize {
    whitespace
        .replace("\r\n", "\n")
        .matches(['\n', '\r'])
        .count()
}

/// The comments that travel with the import ending in `semi`, where they
/// end, and the first token after them that is not whitespace.
fn trailing(semi: &SyntaxToken) -> (Vec<(bool, String)>, TextSize, Option<SyntaxToken>) {
    let mut comments = Vec::new();
    let mut end = semi.text_range().end();
    let mut same_line = true;
    // Whether a `//` comment here would still belong to the import.
    let mut open = true;
    let mut token = semi.next_token();
    while let Some(next) = token {
        match next.kind() {
            WHITESPACE => {
                let breaks = line_breaks(next.text());
                if breaks > 0 {
                    // A blank line ends the comments, as does a comment
                    // that does not start its line.
                    if breaks > 1 || !next.text().ends_with(['\n', '\r']) {
                        open = false;
                    }
                    same_line = false;
                }
            }
            LINE_COMMENT if open => {
                comments.push((same_line, next.text().trim_end().to_string()));
                end = next.text_range().end();
            }
            _ => return (comments, end, Some(next)),
        }
        token = next.next_token();
    }
    (comments, end, None)
}

/// The edits that remove unused imports and sort the import section.
pub(crate) fn fix(text: &str, root: &SyntaxNode) -> Vec<Edit> {
    let imports: Vec<SyntaxNode> = root
        .children()
        .filter(|child| child.kind() == IMPORT_DECL)
        .collect();
    if imports.is_empty() {
        return Vec::new();
    }
    let mut used = code_names(root);
    javadoc_names(root, &mut used);
    let package = package_name(root);

    let mut entries = Vec::new();
    let mut contiguous = true;
    let mut section_end = TextSize::default();
    for (i, import) in imports.iter().enumerate() {
        let (Some(first), Some(semi)) = (first_token(import), last_token(import)) else {
            return Vec::new();
        };
        let (is_static, name) = import_name(import);
        let keep = !is_redundant(is_static, &name, package.as_deref())
            && (name.ends_with('*')
                || name
                    .rsplit('.')
                    .next()
                    .is_some_and(|simple| used.contains(simple)));
        let (comments, end, next) = trailing(&semi);
        if let Some(following) = imports.get(i + 1) {
            let next_start = next.map(|token| token.text_range().start());
            contiguous &= next_start == first_token(following).map(|t| t.text_range().start());
        }
        section_end = end;
        entries.push(Entry {
            is_static,
            name,
            range: TextRange::new(first.text_range().start(), semi.text_range().end()),
            comments,
            keep,
        });
    }

    if !contiguous {
        return entries
            .iter()
            .filter(|entry| !entry.keep)
            .map(|entry| Edit {
                range: entry.range,
                text: String::new(),
            })
            .collect();
    }

    // Comments of removed imports, with no kept import before them.
    let mut top: Vec<String> = Vec::new();
    let mut kept: Vec<Entry> = Vec::new();
    for entry in entries.drain(..) {
        if entry.keep {
            kept.push(entry);
            continue;
        }
        let moved = entry.comments.into_iter().map(|(_, comment)| comment);
        match kept.last_mut() {
            Some(last) => last.comments.extend(moved.map(|comment| (false, comment))),
            None => top.extend(moved),
        }
    }
    kept.sort_by(|a, b| (!a.is_static, &a.name).cmp(&(!b.is_static, &b.name)));
    // Of duplicates, the first stays, and keeps the comments of the others.
    let mut unique: Vec<Entry> = Vec::new();
    for entry in kept {
        match unique.last_mut() {
            Some(last) if last.is_static == entry.is_static && last.name == entry.name => {
                last.comments.extend(
                    entry
                        .comments
                        .into_iter()
                        .map(|(_, comment)| (false, comment)),
                )
            }
            _ => unique.push(entry),
        }
    }

    let mut out = String::new();
    for comment in &top {
        out.push_str(comment);
        out.push('\n');
    }
    for (i, entry) in unique.iter().enumerate() {
        if i > 0 {
            out.push('\n');
            if unique[i - 1].is_static != entry.is_static {
                out.push('\n');
            }
        }
        out.push_str(&text[entry.range]);
        for (same_line, comment) in &entry.comments {
            out.push_str(if *same_line { " " } else { "\n" });
            out.push_str(comment);
        }
    }
    let start = imports
        .first()
        .and_then(first_token)
        .map_or_else(TextSize::default, |token| token.text_range().start());
    let after = &text[usize::from(section_end)..];
    let rest = after.trim_start();
    let rest_start = section_end + TextSize::of(&after[..after.len() - rest.len()]);
    if !out.is_empty() {
        out.push_str(if rest.is_empty() { "\n" } else { "\n\n" });
    }
    let range = TextRange::new(start, rest_start);
    if text[range] == out {
        return Vec::new();
    }
    vec![Edit { range, text: out }]
}
