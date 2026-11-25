use std::collections::HashSet;

use weavejig_syntax::SyntaxKind::*;

use crate::ast;
use crate::rules::util::delete_lines;
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// An import that changes nothing: of a type in `java.lang`, of a type in
/// the file's own package, or a repeat of an earlier import.
pub(crate) struct RedundantImport;

static META: RuleMeta = RuleMeta::new(
    "redundant-import",
    "imports from `java.lang`, from the file's own package, or repeated, should be removed",
)
.checkstyle("RedundantImport")
.fixable();

impl Rule for RedundantImport {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_file(&self, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let package = ast::package_name(ctx.root);
        let mut seen = HashSet::new();
        for (node, import) in ast::imports(ctx.root) {
            let text = ast::compact(&node);
            let why = if !seen.insert(text) {
                Some("is a duplicate")
            } else if import.is_static {
                None
            } else if import.qualifier() == "java.lang" {
                Some("is from `java.lang`, which is always imported")
            } else if !package.is_empty() && import.qualifier() == package {
                Some("is from this file's own package")
            } else {
                None
            };
            let Some(why) = why else {
                continue;
            };
            let star = if import.is_wildcard { ".*" } else { "" };
            debug_assert_eq!(node.kind(), IMPORT_DECL);
            out.push(
                Diagnostic::new(
                    &META,
                    node.text_range(),
                    format!("import of `{}{star}` {why}", import.name),
                )
                .with_fix(
                    "remove the import",
                    vec![delete_lines(ctx, node.text_range())],
                ),
            );
        }
    }
}
