//! Helpers for reading the syntax tree, shared by the rules.
//!
//! The tree is untyped: a declaration's name is an `IDENT` token among its
//! children, its modifiers a `MODIFIERS` node. These functions give those
//! shapes names, so that rules do not each index into children.

use weavejig_syntax::{
    NodeOrToken, SyntaxKind, SyntaxKind::*, SyntaxNode, SyntaxToken, child_node,
    significant_children,
};

/// The direct child tokens of `node` that are not whitespace or comments.
pub(crate) fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
}

/// The `MODIFIERS` of a declaration, parameter or variable.
pub(crate) fn modifiers(node: &SyntaxNode) -> Option<SyntaxNode> {
    child_node(node, MODIFIERS)
}

/// The modifier keyword `kind` of `node`, if it has it.
pub(crate) fn modifier(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    tokens(&modifiers(node)?).find(|token| token.kind() == kind)
}

pub(crate) fn has_modifier(node: &SyntaxNode, kind: SyntaxKind) -> bool {
    modifier(node, kind).is_some()
}

/// The annotations of a declaration, parameter or variable.
pub(crate) fn annotations(node: &SyntaxNode) -> Vec<SyntaxNode> {
    modifiers(node)
        .map(|modifiers| {
            modifiers
                .children()
                .filter(|child| child.kind() == ANNOTATION)
                .collect()
        })
        .unwrap_or_default()
}

/// The name of an annotation as written, such as `Override` or
/// `java.lang.Override`.
pub(crate) fn annotation_name(annotation: &SyntaxNode) -> String {
    child_node(annotation, NAME)
        .map(|name| compact(&name))
        .unwrap_or_default()
}

/// Whether `node` has an annotation whose simple name is `simple`.
pub(crate) fn has_annotation(node: &SyntaxNode, simple: &str) -> bool {
    annotations(node).iter().any(|annotation| {
        let name = annotation_name(annotation);
        name == simple
            || (name.ends_with(simple) && name[..name.len() - simple.len()].ends_with('.'))
    })
}

/// The text of `node` without whitespace and comments.
pub(crate) fn compact(node: &SyntaxNode) -> String {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.text().to_string())
        .collect()
}

/// The name of a declaration: of a type, method, constructor, enum constant,
/// variable declarator, parameter, type parameter or pattern.
pub(crate) fn name(node: &SyntaxNode) -> Option<SyntaxToken> {
    let mut idents = tokens(node).filter(|token| token.kind() == IDENT);
    match node.kind() {
        // `record` is a contextual keyword, lexed as an identifier.
        RECORD_DECL => idents.nth(1),
        _ => idents.next(),
    }
}

pub(crate) fn name_text(node: &SyntaxNode) -> String {
    name(node)
        .map(|token| token.text().to_string())
        .unwrap_or_default()
}

/// The nearest enclosing type declaration, or the body of an anonymous class
/// or enum constant, whichever is innermost.
pub(crate) fn enclosing_type(node: &SyntaxNode) -> Option<SyntaxNode> {
    node.ancestors().skip(1).find(|ancestor| {
        ancestor.kind().is_type_decl()
            || matches!(ancestor.kind(), NEW_EXPR | ENUM_CONSTANT)
                && child_node(ancestor, CLASS_BODY).is_some()
    })
}

/// The type a member is declared in: the parent of its `CLASS_BODY`.
pub(crate) fn owner(member: &SyntaxNode) -> Option<SyntaxNode> {
    member
        .parent()
        .filter(|body| body.kind() == CLASS_BODY)
        .and_then(|body| body.parent())
}

/// Whether code outside the package can see `class`: it and every type
/// around it is public or protected. Members of an interface are public
/// without saying so.
pub(crate) fn is_visible_outside(class: &SyntaxNode) -> bool {
    let mut current = class.clone();
    loop {
        let owner = owner(&current);
        let implicit = owner.as_ref().is_some_and(is_interface);
        if !implicit && !has_modifier(&current, PUBLIC_KW) && !has_modifier(&current, PROTECTED_KW)
        {
            return false;
        }
        match owner {
            Some(owner) if owner.kind().is_type_decl() => current = owner,
            // A local or anonymous class is not visible at all.
            Some(_) => return false,
            None => {
                return current
                    .parent()
                    .is_some_and(|p| p.kind() == COMPILATION_UNIT);
            }
        }
    }
}

/// The members of a type declaration (or of an anonymous class body).
pub(crate) fn members(ty: &SyntaxNode) -> Vec<SyntaxNode> {
    child_node(ty, CLASS_BODY)
        .map(|body| body.children().collect())
        .unwrap_or_default()
}

/// Whether members of `ty` are implicitly public, as in an interface or an
/// annotation type.
pub(crate) fn is_interface(ty: &SyntaxNode) -> bool {
    matches!(ty.kind(), INTERFACE_DECL | ANNOTATION_TYPE_DECL)
}

/// The parameters of a method, constructor, record or lambda.
pub(crate) fn params(node: &SyntaxNode) -> Vec<SyntaxNode> {
    node.children()
        .find(|child| matches!(child.kind(), PARAM_LIST | LAMBDA_PARAMS))
        .map(|list| {
            list.children()
                .filter(|child| child.kind() == PARAM)
                .collect()
        })
        .unwrap_or_default()
}

/// The body of a method, constructor, lambda, loop or `catch`.
pub(crate) fn body(node: &SyntaxNode) -> Option<SyntaxNode> {
    node.children().filter(|child| child.kind() == BLOCK).last()
}

/// The declared type of a field, variable, parameter or method.
pub(crate) fn declared_type(node: &SyntaxNode) -> Option<SyntaxNode> {
    node.children().find(|child| is_type(child.kind()))
}

pub(crate) fn is_type(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        PRIMITIVE_TYPE | NAMED_TYPE | ARRAY_TYPE | UNION_TYPE | INTERSECTION_TYPE | WILDCARD_TYPE
    )
}

/// The statements of a block, switch group or switch rule: its child nodes
/// other than case labels.
pub(crate) fn statements(node: &SyntaxNode) -> Vec<SyntaxNode> {
    node.children()
        .filter(|child| child.kind() != CASE_LABEL)
        .collect()
}

/// `expr` without any parentheses around it.
pub(crate) fn unparenthesized(expr: &SyntaxNode) -> SyntaxNode {
    let mut current = expr.clone();
    while current.kind() == PAREN_EXPR {
        match current.children().next() {
            Some(inner) => current = inner,
            None => break,
        }
    }
    current
}

/// The literal token of a `LITERAL` expression.
pub(crate) fn literal(expr: &SyntaxNode) -> Option<SyntaxToken> {
    (expr.kind() == LITERAL)
        .then(|| tokens(expr).next())
        .flatten()
}

pub(crate) fn is_literal(expr: &SyntaxNode, kind: SyntaxKind) -> bool {
    literal(expr).is_some_and(|token| token.kind() == kind)
}

/// The operator of a binary, prefix, postfix or assignment expression: its
/// direct child tokens, which are several for a shift (`>` `>`).
pub(crate) fn operator(expr: &SyntaxNode) -> String {
    tokens(expr).map(|token| token.text().to_string()).collect()
}

/// The name a method call invokes, and its argument list.
pub(crate) fn call(expr: &SyntaxNode) -> Option<(SyntaxToken, SyntaxNode)> {
    if expr.kind() != METHOD_CALL {
        return None;
    }
    let name = tokens(expr).filter(|token| token.kind() == IDENT).last()?;
    Some((name, child_node(expr, ARG_LIST)?))
}

/// The arguments of an argument list.
pub(crate) fn arguments(list: &SyntaxNode) -> Vec<SyntaxNode> {
    list.children().collect()
}

/// The receiver of a method call or field access: the expression before the
/// last `.`, if there is one.
pub(crate) fn receiver(expr: &SyntaxNode) -> Option<SyntaxNode> {
    match significant_children(expr).next()? {
        NodeOrToken::Node(node) if node.kind() != ARG_LIST && node.kind() != TYPE_ARGS => {
            Some(node)
        }
        _ => None,
    }
}

/// The Javadoc comment of a declaration: the last `/** */` comment between
/// the previous sibling and the declaration.
pub(crate) fn javadoc(node: &SyntaxNode) -> Option<SyntaxToken> {
    let mut previous = node.prev_sibling_or_token();
    while let Some(element) = previous {
        let token = element.as_token()?;
        match token.kind() {
            BLOCK_COMMENT if crate::javadoc::is_javadoc(token.text()) => {
                return Some(token.clone());
            }
            WHITESPACE | BLOCK_COMMENT | LINE_COMMENT => {}
            _ => return None,
        }
        previous = token.prev_sibling_or_token();
    }
    None
}

/// An import declaration, taken apart.
pub(crate) struct Import {
    pub(crate) is_static: bool,
    /// The dotted name, without `.*`.
    pub(crate) name: String,
    pub(crate) is_wildcard: bool,
}

impl Import {
    /// The name the import brings into scope; `None` for a wildcard.
    pub(crate) fn simple_name(&self) -> Option<&str> {
        (!self.is_wildcard).then(|| self.name.rsplit('.').next().unwrap_or(&self.name))
    }

    /// What the name is imported from: the package of a type import, the
    /// type of a static one, and the whole name of a wildcard.
    pub(crate) fn qualifier(&self) -> &str {
        if self.is_wildcard {
            return &self.name;
        }
        self.name
            .rsplit_once('.')
            .map_or("", |(qualifier, _)| qualifier)
    }
}

pub(crate) fn import(node: &SyntaxNode) -> Option<Import> {
    if node.kind() != IMPORT_DECL {
        return None;
    }
    Some(Import {
        is_static: tokens(node).any(|token| token.kind() == STATIC_KW),
        name: compact(&child_node(node, NAME)?),
        is_wildcard: tokens(node).any(|token| token.kind() == STAR),
    })
}

/// The imports of a compilation unit.
pub(crate) fn imports(root: &SyntaxNode) -> Vec<(SyntaxNode, Import)> {
    root.children()
        .filter_map(|child| Some((child.clone(), import(&child)?)))
        .collect()
}

/// The name of the file's package; empty for the unnamed package.
pub(crate) fn package_name(root: &SyntaxNode) -> String {
    child_node(root, PACKAGE_DECL)
        .and_then(|package| child_node(&package, NAME))
        .map(|name| compact(&name))
        .unwrap_or_default()
}
