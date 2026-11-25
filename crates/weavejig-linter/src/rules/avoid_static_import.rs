use crate::ast;
use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// Static imports hide which class a member comes from.
pub(crate) struct AvoidStaticImport;

static META: RuleMeta = RuleMeta::new(
    "avoid-static-import",
    "static imports should be avoided",
)
.checkstyle("AvoidStaticImport")
.opt_in()
.options(&[OptionSpec {
    name: "excludes",
    default: OptionDefault::List(&[]),
    help: "members that may be imported statically: `a.B.member`, or `a.B.*` for all of a class",
}]);

impl Rule for AvoidStaticImport {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_file(&self, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        for (node, import) in ast::imports(ctx.root) {
            if !import.is_static {
                continue;
            }
            let excluded = ctx.options.list("excludes").iter().any(|exclude| {
                match exclude.strip_suffix(".*") {
                    // `a.B.*` allows every member of `a.B`, however imported.
                    Some(class) => import.qualifier() == class,
                    None => !import.is_wildcard && import.name == *exclude,
                }
            });
            if !excluded {
                let star = if import.is_wildcard { ".*" } else { "" };
                out.push(Diagnostic::new(
                    &META,
                    node.text_range(),
                    format!("static import of `{}{star}` should be avoided", import.name),
                ));
            }
        }
    }
}
