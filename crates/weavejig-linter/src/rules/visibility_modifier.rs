use weavejig_syntax::{SyntaxKind::*, SyntaxNode};

use crate::ast;
use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// Fields should be private, and reached through methods: a field anyone
/// can set is a field whose invariants nobody can keep.
///
/// `static final` constants are fine, and so is `serialVersionUID`.
pub(crate) struct VisibilityModifier;

static META: RuleMeta = RuleMeta::new(
    "visibility-modifier",
    "fields should be private unless they are `static final`",
)
.checkstyle("VisibilityModifier")
.opt_in()
.options(&[
    OptionSpec {
        name: "package-allowed",
        default: OptionDefault::Bool(false),
        help: "allow package-private fields",
    },
    OptionSpec {
        name: "protected-allowed",
        default: OptionDefault::Bool(false),
        help: "allow protected fields",
    },
    OptionSpec {
        name: "public-member-pattern",
        default: OptionDefault::Regex("^serialVersionUID$"),
        help: "names of fields that may be public",
    },
    OptionSpec {
        name: "allow-public-final-fields",
        default: OptionDefault::Bool(false),
        help: "allow public fields that are `final`",
    },
    OptionSpec {
        name: "ignore-annotations",
        default: OptionDefault::List(&[
            "com.google.common.annotations.VisibleForTesting",
            "org.junit.Rule",
            "org.junit.ClassRule",
        ]),
        help: "fully qualified annotations whose fields may have any visibility",
    },
]);

/// Whether the annotation, as written, is `canonical`: written in full, or
/// by its simple name and imported, or defined in `java.lang`.
fn is_annotation(written: &str, canonical: &str, root: &SyntaxNode) -> bool {
    if written == canonical {
        return true;
    }
    let Some((package, simple)) = canonical.rsplit_once('.') else {
        return false;
    };
    written == simple
        && (package == "java.lang"
            || ast::imports(root).iter().any(|(_, import)| {
                !import.is_static
                    && if import.is_wildcard {
                        import.name == package
                    } else {
                        import.name == canonical
                    }
            }))
}

impl Rule for VisibilityModifier {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        // The fields of an interface are constants.
        if node.kind() != FIELD_DECL
            || node
                .parent()
                .and_then(|body| body.parent())
                .is_none_or(|owner| ast::is_interface(&owner))
        {
            return;
        }
        let has = |kind| ast::has_modifier(node, kind);
        if has(PRIVATE_KW) || (has(STATIC_KW) && has(FINAL_KW)) {
            return;
        }
        let options = ctx.options;
        let is_public = has(PUBLIC_KW);
        if (has(PROTECTED_KW) && options.bool("protected-allowed"))
            || (!is_public && !has(PROTECTED_KW) && options.bool("package-allowed"))
            || (is_public && has(FINAL_KW) && options.bool("allow-public-final-fields"))
        {
            return;
        }
        let ignored = ast::annotations(node).iter().any(|annotation| {
            let written = ast::annotation_name(annotation);
            options
                .list("ignore-annotations")
                .iter()
                .any(|canonical| is_annotation(&written, canonical, ctx.root))
        });
        if ignored {
            return;
        }
        for declarator in node
            .children()
            .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        {
            let Some(name) = ast::name(&declarator) else {
                continue;
            };
            if is_public && options.matches("public-member-pattern", name.text()) {
                continue;
            }
            out.push(Diagnostic::new(
                &META,
                name.text_range(),
                format!("field `{}` should be private", name.text()),
            ));
        }
    }
}
