use crate::ast;
use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// Imports from packages that code should not depend on. By default `sun.*`,
/// which is internal to the JDK and may change or disappear.
pub(crate) struct IllegalImport;

static META: RuleMeta = RuleMeta::new(
    "illegal-import",
    "imports from forbidden packages (by default `sun.*`) should be avoided",
)
.checkstyle("IllegalImport")
.options(&[
    OptionSpec {
        name: "packages",
        default: OptionDefault::List(&["sun"]),
        help: "packages whose members, and those of their subpackages, must not be imported",
    },
    OptionSpec {
        name: "classes",
        default: OptionDefault::List(&[]),
        help: "fully qualified names that must not be imported",
    },
]);

impl Rule for IllegalImport {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_file(&self, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        for (node, import) in ast::imports(ctx.root) {
            let from_package = ctx.options.list("packages").iter().any(|package| {
                import
                    .name
                    .strip_prefix(package.as_str())
                    .is_some_and(|rest| rest.starts_with('.'))
            });
            let is_class = ctx.options.list("classes").contains(&import.name);
            if from_package || is_class {
                out.push(Diagnostic::new(
                    &META,
                    node.text_range(),
                    format!("import of `{}` is not allowed", import.name),
                ));
            }
        }
    }
}
