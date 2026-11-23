//! The safety net: formatted output must be the input with different
//! whitespace, and nothing else, apart from the few rewrites
//! google-java-format makes: imports sorted and unused ones removed,
//! modifier keywords reordered, and long strings split.

use std::collections::HashMap;

use weavejig_syntax::{SyntaxKind, SyntaxNode, TextSize, lex, parse};

use crate::{imports, modifiers, strings};

/// What a source file says, ignoring layout.
#[derive(Debug, PartialEq, Eq)]
struct Fingerprint {
    /// Every token outside import declarations, in order, except that each
    /// run of string literals joined by `+` counts as one token: long
    /// strings are split and their pieces merged again, which leaves the
    /// literals' joined text, and so the string's value, as it was. Each
    /// run of modifier keywords with no annotation between them counts as
    /// one token, too, listing them alphabetically, since they are sorted.
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

/// The runs of modifier keywords, by start offset: where each ends, and its
/// keywords in alphabetical order.
fn modifier_runs(root: &SyntaxNode) -> HashMap<TextSize, (TextSize, String)> {
    modifiers::runs(root)
        .into_iter()
        .filter_map(|run| {
            let start = run.first()?.range.start();
            let end = run.last()?.range.end();
            let mut keywords: Vec<String> = run.into_iter().map(|keyword| keyword.text).collect();
            keywords.sort();
            Some((start, (end, keywords.join(" "))))
        })
        .collect()
}

fn fingerprint(text: &str) -> Fingerprint {
    let root = parse(text).syntax();
    let runs = strings::run_tokens(&root);
    let modifier_runs = modifier_runs(&root);
    let (tokens, _) = lex(text);
    let mut fingerprint = Fingerprint {
        tokens: Vec::new(),
        imports: Vec::new(),
        comments: Vec::new(),
    };
    let mut import: Option<String> = None;
    let mut offset = 0;
    // The end of the run of string literals or modifiers being skipped.
    let mut run_end = 0;
    for token in tokens {
        let len = u32::from(token.len) as usize;
        let token_text = &text[offset..offset + len];
        let start = offset;
        offset += len;
        match token.kind {
            SyntaxKind::WHITESPACE => {}
            kind if kind.is_comment() => comment_words(token_text, &mut fingerprint.comments),
            _ if start < run_end => {}
            _ if modifier_runs.contains_key(&TextSize::try_from(start).unwrap_or_default()) => {
                let at = TextSize::try_from(start).unwrap_or_default();
                let (end, keywords) = &modifier_runs[&at];
                run_end = usize::from(*end);
                fingerprint.tokens.push(keywords.clone());
            }
            SyntaxKind::STRING_LITERAL if import.is_none() => {
                let at = TextSize::try_from(start).unwrap_or_default();
                match runs.get(&at) {
                    Some((end, run)) => {
                        run_end = usize::from(*end);
                        fingerprint.tokens.push(run.clone());
                    }
                    None => fingerprint.tokens.push(token_text.to_string()),
                }
            }
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
    check_imports(&before.imports, &after.imports, formatted)?;
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

/// Checks that the imports of the formatted file are those of the original,
/// except for imports that are unused in the formatted file, or that do
/// nothing, which may be removed. A wildcard import must stay.
fn check_imports(before: &[String], after: &[String], formatted: &str) -> Result<(), String> {
    if after.iter().any(|import| !before.contains(import)) {
        return Err("imports would change".to_string());
    }
    let removed: Vec<&String> = before
        .iter()
        .filter(|import| !after.contains(import))
        .collect();
    if removed.is_empty() {
        return Ok(());
    }
    let root = parse(formatted).syntax();
    let used = imports::code_names(&root);
    let package = imports::package_name(&root);
    for import in removed {
        // `static a . B . c ` or `a . B `, as the fingerprint keeps them.
        let mut parts: Vec<&str> = import.split_whitespace().collect();
        let is_static = parts.first() == Some(&"static");
        if is_static {
            parts.remove(0);
        }
        let name: String = parts.concat();
        let simple = parts.last().copied().unwrap_or_default();
        if simple == "*" {
            return Err(format!("import `{name}` would be removed"));
        }
        if used.contains(simple) && !imports::is_redundant(is_static, &name, package.as_deref()) {
            return Err(format!("import `{name}` would be removed, but is used"));
        }
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
    fn split_strings_are_fine() {
        assert!(
            check(
                r#"class A { String s = "ab cd"; }"#,
                r#"class A { String s = "ab" + " cd"; }"#
            )
            .is_ok()
        );
        assert!(
            check(
                r#"class A { String s = "a" + "b" + x + "c"; }"#,
                r#"class A { String s = "ab" + x + "c"; }"#
            )
            .is_ok()
        );
        assert!(
            check(
                r#"class A { Object o = f("ab cd"); }"#,
                r#"class A { Object o = f("ab"
            + " cd"); }"#
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
        assert!(check("import a.A; class C { A a; }", "class C { A a; }").is_err());
        assert!(check("import a.*; class C {}", "class C {}").is_err());
        assert!(check("import a.A; class C {}", "import b.A; class C {}").is_err());
        assert!(
            check(
                "class A { int x = a >> b; }",
                "class A { int x = a > > b; }"
            )
            .is_err()
        );
    }

    #[test]
    fn unused_imports_may_go() {
        assert!(check("import a.A; class C {}", "class C {}").is_ok());
        assert!(check("import static a.B.c; class C {}", "class C {}").is_ok());
        assert!(
            check(
                "import a.A; class C { int x = y.A; }",
                "class C { int x = y.A; }"
            )
            .is_ok()
        );
        assert!(
            check(
                "package p; import java.lang.String; import p.D; class C { String s; D d; }",
                "package p; class C { String s; D d; }"
            )
            .is_ok()
        );
        assert!(
            check(
                "import static a.B.c; class C { int x = c(); }",
                "class C { int x = c(); }"
            )
            .is_err()
        );
    }

    #[test]
    fn modifier_order_may_change() {
        assert!(
            check(
                "class A { final static public int X = 1; }",
                "class A { public static final int X = 1; }"
            )
            .is_ok()
        );
        assert!(
            check(
                "class A { final /* c */ static int X; }",
                "class A { static /* c */ final int X; }"
            )
            .is_ok()
        );
        assert!(
            check(
                "non-sealed static class A {}",
                "static non-sealed class A {}"
            )
            .is_ok()
        );
        // Keywords stay on their side of an annotation.
        assert!(
            check(
                "class A { static @D final int X; }",
                "class A { static final @D int X; }"
            )
            .is_err()
        );
        assert!(check("class A { static int X; }", "class A { final int X; }").is_err());
        assert!(
            check(
                "class A { static final int X; }",
                "class A { static static int X; }"
            )
            .is_err()
        );
    }

    #[test]
    fn string_changes_are_caught() {
        let fails = |before: &str, after: &str| {
            check(
                &format!("class A {{ Object o = {before}; }}"),
                &format!("class A {{ Object o = {after}; }}"),
            )
            .is_err()
        };
        assert!(fails(r#""ab""#, r#""ba""#));
        assert!(!fails(r#""ab""#, r#""a" + "b""#));
        // The joined text must be the same, not only the value.
        assert!(fails(r#""\u0041""#, r#""A""#));
        // Joining escapes changes the value.
        assert!(fails(r#""\1" + "2""#, r#""\12""#));
        // A receiver or a cast's operand cannot be split.
        assert!(fails(r#""ab".length()"#, r#""a" + "b".length()"#));
        assert!(fails(r#"(Object) "ab""#, r#"(Object) "a" + "b""#));
        // Nor joined across another operand or operator.
        assert!(fails(r#""a" + x + "b""#, r#""ab" + x"#));
        assert!(fails(r#"x - "a" + "b""#, r#"x - "ab""#));
        assert!(fails(r#""a" + ("b")"#, r#""ab""#));
    }
}
