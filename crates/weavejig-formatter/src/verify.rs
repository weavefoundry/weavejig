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
    /// The characters of all comments, sorted: comments travel with imports
    /// and are re-indented and re-wrapped, so neither order nor whitespace
    /// can be compared. Comment markers, and the tags Javadoc reflow adds
    /// and removes, are left out.
    comments: Vec<char>,
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
            // The compiler reads every line terminator in a text block as
            // `\n` (JLS §3.10.6), so the file's line endings may change them.
            SyntaxKind::TEXT_BLOCK if import.is_none() => fingerprint
                .tokens
                .push(token_text.replace("\r\n", "\n").replace('\r', "\n")),
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
    fingerprint.comments.sort_unstable();
    fingerprint
}

/// Adds the characters of a comment to `chars`, leaving out what re-wrapping
/// a comment may add or remove: whitespace, `//`, `/*`, `*/`, the `*` that
/// starts a line, and the tags Javadoc reflow normalises.
fn comment_words(comment: &str, chars: &mut Vec<char>) {
    // Tags that reflow drops, adds, or rewrites (`<br/>` becomes `<br>`).
    const DROPPED: [&str; 6] = ["<p>", "</p>", "<p/>", "</li>", "</dt>", "</dd>"];
    let is_line_comment = comment.starts_with("//");
    let body = comment
        .strip_prefix("/*")
        .and_then(|body| body.strip_suffix("*/"))
        .unwrap_or(comment);
    for line in body.lines() {
        let line = line.trim();
        let line = if is_line_comment {
            line.trim_start_matches('/')
        } else {
            // One `*` starts the line; any more belong to the text.
            line.strip_prefix('*').unwrap_or(line)
        };
        let mut rest = line;
        'scan: while let Some(first) = rest.chars().next() {
            if first == '<' {
                let lower: String = rest
                    .chars()
                    .take(6)
                    .collect::<String>()
                    .to_ascii_lowercase();
                for tag in DROPPED {
                    if lower.starts_with(tag) {
                        rest = &rest[tag.len()..];
                        continue 'scan;
                    }
                }
                if lower.starts_with("<br")
                    && let Some(end) = rest
                        .find('>')
                        .filter(|&end| rest[3..end].chars().all(|c| c == '/' || c.is_whitespace()))
                {
                    chars.extend("<br>".chars());
                    rest = &rest[end + 1..];
                    continue 'scan;
                }
            }
            if !first.is_whitespace() {
                chars.push(first);
            }
            rest = &rest[first.len_utf8()..];
        }
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
