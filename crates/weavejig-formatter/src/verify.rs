//! The safety net: formatted output must be the input with different
//! whitespace, and nothing else.

use weavejig_syntax::{SyntaxKind, lex, parse};

/// What a source file says, ignoring layout.
#[derive(Debug, PartialEq, Eq)]
struct Fingerprint {
    /// Every token outside import declarations, in order.
    tokens: Vec<String>,
    /// Import declarations, sorted and without duplicates, since the
    /// formatter reorders and deduplicates them.
    imports: Vec<String>,
    /// The words of all comments, sorted: comments travel with imports, and
    /// are re-indented and re-wrapped. Comment markers and paragraph tags,
    /// which wrapping adds and removes, are left out.
    comments: Vec<String>,
}

fn fingerprint(text: &str) -> Fingerprint {
    let (tokens, _) = lex(text);
    let mut fingerprint = Fingerprint {
        tokens: Vec::new(),
        imports: Vec::new(),
        comments: Vec::new(),
    };
    let mut import: Option<String> = None;
    let mut offset = 0;
    for token in tokens {
        let len = u32::from(token.len) as usize;
        let token_text = &text[offset..offset + len];
        offset += len;
        match token.kind {
            SyntaxKind::WHITESPACE => {}
            kind if kind.is_comment() => comment_words(token_text, &mut fingerprint.comments),
            // `import` is a reserved word: it only ever starts an import.
            SyntaxKind::IMPORT_KW => import = Some(String::new()),
            SyntaxKind::SEMI if import.is_some() => fingerprint.imports.extend(import.take()),
            _ => match &mut import {
                Some(import) => {
                    import.push_str(token_text);
                    import.push(' ');
                }
                None => fingerprint.tokens.push(token_text.to_string()),
            },
        }
    }
    fingerprint.imports.extend(import);
    fingerprint.imports.sort();
    fingerprint.imports.dedup();
    fingerprint.comments.sort();
    fingerprint
}

/// Adds the words of a comment to `words`, leaving out what re-wrapping a
/// comment may add or remove: `//`, `/*`, `*/`, the `*` that starts a line,
/// and `<p>` tags.
fn comment_words(comment: &str, words: &mut Vec<String>) {
    for line in comment.lines() {
        let mut line = line.trim();
        if line.starts_with("//") {
            line = line.trim_start_matches('/');
        } else if let Some(rest) = line.strip_prefix("/*") {
            line = rest;
        }
        let line = line.strip_suffix("*/").unwrap_or(line);
        let line = line.trim_start().trim_start_matches('*');
        let line = line
            .replace("<p>", " ")
            .replace("</p>", " ")
            .replace("<P>", " ")
            .replace("</P>", " ");
        words.extend(line.split_whitespace().map(str::to_string));
    }
}

/// Checks that `formatted` differs from `original` only in layout. On
/// failure, returns a description of the first difference.
pub(crate) fn check(original: &str, formatted: &str) -> Result<(), String> {
    let before = fingerprint(original);
    let after = fingerprint(formatted);
    if before.tokens != after.tokens {
        let index = before
            .tokens
            .iter()
            .zip(&after.tokens)
            .position(|(a, b)| a != b)
            .unwrap_or(before.tokens.len().min(after.tokens.len()));
        let show = |tokens: &[String]| tokens.get(index).cloned().unwrap_or("end of file".into());
        return Err(format!(
            "token {index} would change from `{}` to `{}`",
            show(&before.tokens),
            show(&after.tokens)
        ));
    }
    if before.imports != after.imports {
        return Err("imports would change".to_string());
    }
    if before.comments != after.comments {
        return Err("comments would change".to_string());
    }
    // Equal tokens can still parse differently if they were glued together
    // or pulled apart (`> >` is not `>>`).
    if !parse(formatted).is_ok() {
        return Err("output would not parse".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_changes_are_fine() {
        assert!(check("class A{int x;}", "class A {\n  int x;\n}\n").is_ok());
        assert!(
            check(
                "import b.B;import a.A;import a.A;class C{}",
                "import a.A;\nimport b.B;\n\nclass C {}\n"
            )
            .is_ok()
        );
        assert!(
            check(
                "/**\n   * Doc.\n   */\nclass A {}",
                "/**\n * Doc.\n */\nclass A {}\n"
            )
            .is_ok()
        );
    }

    #[test]
    fn content_changes_are_caught() {
        assert!(check("class A { int x; }", "class A { int y; }").is_err());
        assert!(check("class A { int x; }", "class A { int x }").is_err());
        assert!(check("class A { int x; }", "class A { int x; ; }").is_err());
        assert!(check("class A { /* a */ }", "class A {}").is_err());
        assert!(check("import a.A; class C {}", "class C {}").is_err());
        assert!(
            check(
                "class A { int x = a >> b; }",
                "class A { int x = a > > b; }"
            )
            .is_err()
        );
    }
}
