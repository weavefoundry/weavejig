use weavejig_syntax::{SyntaxKind::*, SyntaxNode, first_token};

use crate::ast;
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// A file is named after its outer type: `Foo.java` declares `Foo`.
///
/// If the first type has another name, a later public type excuses it:
/// the compiler requires that one to match.
pub(crate) struct OuterTypeFilename;

static META: RuleMeta = RuleMeta::new(
    "outer-type-filename",
    "the outer type should have the name of its file",
)
.checkstyle("OuterTypeFilename");

impl Rule for OuterTypeFilename {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check_file(&self, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let Some(stem) = ctx
            .path
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".java"))
        else {
            return;
        };
        let mut types = ctx
            .root
            .children()
            .filter(|child| child.kind().is_type_decl());
        let Some(first) = types.next() else {
            return;
        };
        let name = ast::name_text(&first);
        if name == stem || types.any(|ty: SyntaxNode| ast::has_modifier(&ty, PUBLIC_KW)) {
            return;
        }
        let Some(token) = first_token(&first) else {
            return;
        };
        out.push(Diagnostic::new(
            &META,
            token.text_range(),
            format!("type `{name}` should be declared in a file named `{name}.java`"),
        ));
    }
}
