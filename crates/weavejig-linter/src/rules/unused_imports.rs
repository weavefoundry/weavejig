use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use weavejig_syntax::{SyntaxKind::*, SyntaxNode, TextRange, TextSize, prev_significant_token};

use crate::ast;
use crate::javadoc;
use crate::options::{OptionDefault, OptionSpec};
use crate::rules::util::delete_lines;
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// An import whose name nothing in the file uses.
///
/// A name counts as used if it appears in code, or in a Javadoc reference
/// such as `{@link Name}` or `@throws Name`. Wildcard imports are never
/// reported: what they bring in cannot be known from the file alone. An
/// import of a type in `java.lang` is always reported: nothing needs it.
pub(crate) struct UnusedImports;

static META: RuleMeta = RuleMeta::new(
    "unused-imports",
    "imports that nothing uses should be removed",
)
.checkstyle("UnusedImports")
.fixable()
.options(&[OptionSpec {
    name: "process-javadoc",
    default: OptionDefault::Bool(true),
    help: "whether a reference in a Javadoc comment counts as a use",
}]);

impl Rule for UnusedImports {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_file(&self, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let imports = ast::imports(ctx.root);
        if imports.iter().all(|(_, import)| import.is_wildcard) {
            return;
        }
        let used = used_names(ctx.root, ctx.options.bool("process-javadoc"));
        for (node, import) in imports {
            let Some(name) = import.simple_name() else {
                continue;
            };
            // An import of a `java.lang` type is never needed, used or not.
            let needless = !import.is_static && import.qualifier() == "java.lang";
            if used.contains(name) && !needless {
                continue;
            }
            out.push(
                Diagnostic::new(
                    &META,
                    node.text_range(),
                    format!("unused import `{}`", import.name),
                )
                .with_fix(
                    "remove the import",
                    vec![delete_lines(ctx, node.text_range())],
                ),
            );
        }
    }
}

/// Every name the file could be using an import for: identifiers that are
/// not qualified by something before them, and names in Javadoc references.
///
/// A name does not count where it means a type the file itself declares:
/// inside the scope of `class Entry`, `Entry` is not the imported one.
fn used_names(root: &SyntaxNode, with_javadoc: bool) -> HashSet<String> {
    let mut declared: HashMap<String, Vec<TextRange>> = HashMap::new();
    for node in root.descendants().filter(|node| node.kind().is_type_decl()) {
        if let (Some(name), Some(scope)) = (ast::name(&node), node.parent()) {
            declared
                .entry(name.text().to_string())
                .or_default()
                .push(scope.text_range());
        }
    }
    let shadowed = |name: &str, at: TextSize| {
        declared
            .get(name)
            .is_some_and(|scopes| scopes.iter().any(|scope| scope.contains(at)))
    };

    let mut used = HashSet::new();
    let mut token = root.first_token();
    while let Some(current) = token {
        token = current.next_token();
        let at = current.text_range().start();
        match current.kind() {
            IDENT => {
                let Some(parent) = current.parent() else {
                    continue;
                };
                // The names in the imports themselves do not count.
                if parent.kind() == NAME
                    && parent
                        .parent()
                        .is_some_and(|p| matches!(p.kind(), IMPORT_DECL | PACKAGE_DECL))
                {
                    continue;
                }
                // `a.b`: only `a` can be an imported name. Nor is the name
                // of a method being declared a use of anything.
                let qualified = prev_significant_token(&current)
                    .is_some_and(|previous| matches!(previous.kind(), DOT | COLON_COLON));
                if !qualified && parent.kind() != METHOD_DECL && !shadowed(current.text(), at) {
                    used.insert(current.text().to_string());
                }
            }
            BLOCK_COMMENT if with_javadoc && javadoc::is_javadoc(current.text()) => {
                let mut names = HashSet::new();
                javadoc_references(current.text(), &mut names);
                used.extend(names.into_iter().filter(|name| !shadowed(name, at)));
            }
            _ => {}
        }
    }
    used
}

/// A possibly qualified Java name.
const NAME_PATTERN: &str = r"[\p{L}_$][\p{L}\p{N}_$]*(?:\.[\p{L}_$][\p{L}\p{N}_$]*)*";

/// What a Javadoc tag refers to: `{@link Ref}`, `@see Ref`, and so on.
static REFERENCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?s)\{@(?:link|linkplain|value)\s+([^}]*)\}|(?m:^)@(?:see|throws|exception)\s+([^\n]*)",
    )
    .expect("valid pattern")
});

/// The types of a referenced member's parameters: `(A, b.C)`.
static ARGUMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"[(,]\s*({NAME_PATTERN})")).expect("valid pattern"));

static FIRST_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("^{NAME_PATTERN}")).expect("valid pattern"));

/// Adds the names Javadoc references in `comment` use: for `a.B#c(D, e.F)`,
/// they are `a`, `D` and `e`, the first part of each name.
fn javadoc_references(comment: &str, used: &mut HashSet<String>) {
    let first_part = |name: &str| name.split('.').next().unwrap_or(name).to_string();
    // Without the asterisks that start its lines, so that a reference may
    // run over a line break.
    let text = javadoc::lines(comment).join("\n");
    for captures in REFERENCE.captures_iter(&text) {
        let Some(reference) = captures.get(1).or_else(|| captures.get(2)) else {
            continue;
        };
        let reference = reference.as_str().trim_start();
        if let Some(name) = FIRST_NAME.find(reference) {
            used.insert(first_part(name.as_str()));
        }
        for argument in ARGUMENT.captures_iter(reference) {
            used.insert(first_part(&argument[1]));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references() {
        let mut used = HashSet::new();
        javadoc_references(
            "/**\n * See {@link Foo#bar(Baz, java.util.List)} and {@linkplain\n * Map.Entry entries}.\n \
             * {@value Limits#MAX}\n * @see Other\n * @throws Problem if so\n * @see <a href=\"x\">x</a>\n \
             * @param Unrelated x\n */",
            &mut used,
        );
        let mut names: Vec<_> = used.into_iter().collect();
        names.sort();
        assert_eq!(
            names,
            ["Baz", "Foo", "Limits", "Map", "Other", "Problem", "java"]
        );
    }
}
