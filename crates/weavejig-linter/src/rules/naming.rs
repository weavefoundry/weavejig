//! Naming conventions. Each rule picks the names it is about and checks
//! them against its `format` option, a regular expression.

use weavejig_syntax::{SyntaxKind::*, SyntaxNode, SyntaxToken, child_node, child_token};

use crate::ast;
use crate::options::{OptionDefault, OptionSpec};
use crate::{Context, Diagnostic, Rule, RuleMeta};

/// A naming rule: `names` gives the names a node declares that the rule is
/// about, and `what` says what they are names of.
pub(crate) struct NameRule {
    meta: &'static RuleMeta,
    what: &'static str,
    names: fn(&SyntaxNode) -> Vec<SyntaxToken>,
}

impl Rule for NameRule {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        for name in (self.names)(node) {
            if !ctx.options.matches("format", name.text()) {
                out.push(Diagnostic::new(
                    self.meta,
                    name.text_range(),
                    format!(
                        "{} name `{}` should match `{}`",
                        self.what,
                        name.text(),
                        ctx.options
                            .regex("format")
                            .map_or("", |regex| regex.as_str())
                    ),
                ));
            }
        }
    }
}

const fn format(default: &'static str) -> [OptionSpec; 1] {
    [OptionSpec {
        name: "format",
        default: OptionDefault::Regex(default),
        help: "what the name must match",
    }]
}

const CAMEL: &str = "^[a-z][a-zA-Z0-9]*$";
const CAMEL_OR_UNNAMED: &str = "^([a-z][a-zA-Z0-9]*|_)$";
const ONE_CAPITAL: &str = "^[A-Z]$";

fn declarators(decl: &SyntaxNode) -> Vec<SyntaxToken> {
    decl.children()
        .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        .filter_map(|declarator| ast::name(&declarator))
        .collect()
}

fn in_interface(member: &SyntaxNode) -> bool {
    ast::owner(member).is_some_and(|owner| ast::is_interface(&owner))
}

/// The declarators of a field that is (`constant`) or is not a constant: a
/// `static final` field, or any field of an interface.
fn fields(node: &SyntaxNode, is_static: bool, constant: bool) -> Vec<SyntaxToken> {
    if node.kind() != FIELD_DECL {
        return Vec::new();
    }
    let interface = in_interface(node);
    let node_static = interface || ast::has_modifier(node, STATIC_KW);
    let node_constant = interface || (node_static && ast::has_modifier(node, FINAL_KW));
    if node_constant != constant || (!constant && node_static != is_static) {
        return Vec::new();
    }
    declarators(node)
}

// --- Variables ----------------------------------------------------------------

static CONSTANT_NAME_OPTIONS: [OptionSpec; 1] = format("^[A-Z][A-Z0-9]*(_[A-Z0-9]+)*$");
static CONSTANT_NAME_META: RuleMeta = RuleMeta::new(
    "constant-name",
    "constants (`static final` fields) should be named in upper case with underscores",
)
.checkstyle("ConstantName")
.opt_in()
.options(&CONSTANT_NAME_OPTIONS);
// registry: constant-name => CONSTANT_NAME
pub(crate) static CONSTANT_NAME: NameRule = NameRule {
    meta: &CONSTANT_NAME_META,
    what: "constant",
    names: |node| {
        // A constant of a class declared inside a method is left alone.
        let local = node.ancestors().any(|ancestor| {
            matches!(
                ancestor.kind(),
                METHOD_DECL | CONSTRUCTOR_DECL | INITIALIZER_BLOCK | LAMBDA_EXPR
            )
        });
        if local {
            return Vec::new();
        }
        // Serialization looks for these two by name.
        fields(node, true, true)
            .into_iter()
            .filter(|name| !matches!(name.text(), "serialVersionUID" | "serialPersistentFields"))
            .collect()
    },
};

static STATIC_VARIABLE_NAME_OPTIONS: [OptionSpec; 1] = format(CAMEL);
static STATIC_VARIABLE_NAME_META: RuleMeta = RuleMeta::new(
    "static-variable-name",
    "static fields that are not constants should be named in camel case",
)
.checkstyle("StaticVariableName")
.options(&STATIC_VARIABLE_NAME_OPTIONS);
// registry: static-variable-name => STATIC_VARIABLE_NAME
pub(crate) static STATIC_VARIABLE_NAME: NameRule = NameRule {
    meta: &STATIC_VARIABLE_NAME_META,
    what: "static variable",
    names: |node| fields(node, true, false),
};

static MEMBER_NAME_OPTIONS: [OptionSpec; 1] = format(CAMEL);
static MEMBER_NAME_META: RuleMeta = RuleMeta::new(
    "member-name",
    "instance fields should be named in camel case",
)
.checkstyle("MemberName")
.options(&MEMBER_NAME_OPTIONS);
// registry: member-name => MEMBER_NAME
pub(crate) static MEMBER_NAME: NameRule = NameRule {
    meta: &MEMBER_NAME_META,
    what: "field",
    names: |node| fields(node, false, false),
};

static LOCAL_VARIABLE_NAME_OPTIONS: [OptionSpec; 1] = format(CAMEL_OR_UNNAMED);
static LOCAL_VARIABLE_NAME_META: RuleMeta = RuleMeta::new(
    "local-variable-name",
    "local variables should be named in camel case",
)
.checkstyle("LocalVariableName")
.options(&LOCAL_VARIABLE_NAME_OPTIONS);
// registry: local-variable-name => LOCAL_VARIABLE_NAME
pub(crate) static LOCAL_VARIABLE_NAME: NameRule = NameRule {
    meta: &LOCAL_VARIABLE_NAME_META,
    what: "local variable",
    names: |node| {
        if node.kind() == LOCAL_VAR_DECL && !ast::has_modifier(node, FINAL_KW) {
            declarators(node)
        } else {
            Vec::new()
        }
    },
};

static LOCAL_FINAL_VARIABLE_NAME_OPTIONS: [OptionSpec; 1] = format(CAMEL_OR_UNNAMED);
static LOCAL_FINAL_VARIABLE_NAME_META: RuleMeta = RuleMeta::new(
    "local-final-variable-name",
    "final local variables, and the resources of a `try`, should be named in camel case",
)
.checkstyle("LocalFinalVariableName")
.options(&LOCAL_FINAL_VARIABLE_NAME_OPTIONS);
// registry: local-final-variable-name => LOCAL_FINAL_VARIABLE_NAME
pub(crate) static LOCAL_FINAL_VARIABLE_NAME: NameRule = NameRule {
    meta: &LOCAL_FINAL_VARIABLE_NAME_META,
    what: "final local variable",
    names: |node| match node.kind() {
        LOCAL_VAR_DECL if ast::has_modifier(node, FINAL_KW) => declarators(node),
        // A resource that declares a variable, which is final without
        // saying so.
        RESOURCE if ast::declared_type(node).is_some() && child_token(node, EQ).is_some() => {
            ast::name(node).into_iter().collect()
        }
        PARAM
            if ast::has_modifier(node, FINAL_KW)
                && node.parent().is_some_and(|p| p.kind() == CATCH_CLAUSE) =>
        {
            ast::name(node).into_iter().collect()
        }
        _ => Vec::new(),
    },
};

// --- Parameters ---------------------------------------------------------------

/// The parameter's name, if it is a parameter of what `owner` matches.
fn param_of(node: &SyntaxNode, owner: fn(&SyntaxNode) -> bool) -> Vec<SyntaxToken> {
    if node.kind() != PARAM || child_token(node, THIS_KW).is_some() {
        return Vec::new();
    }
    let Some(parent) = node.parent() else {
        return Vec::new();
    };
    // In a list, the parameter belongs to what the list belongs to; a
    // `catch` and a one-parameter lambda hold theirs directly.
    let holder = match parent.kind() {
        PARAM_LIST | LAMBDA_PARAMS => parent.parent(),
        _ => Some(parent),
    };
    match holder {
        Some(holder) if owner(&holder) => ast::name(node).into_iter().collect(),
        _ => Vec::new(),
    }
}

static PARAMETER_NAME_OPTIONS: [OptionSpec; 1] = format(CAMEL);
static PARAMETER_NAME_META: RuleMeta = RuleMeta::new(
    "parameter-name",
    "parameters of methods and constructors should be named in camel case",
)
.checkstyle("ParameterName")
.options(&PARAMETER_NAME_OPTIONS);
// registry: parameter-name => PARAMETER_NAME
pub(crate) static PARAMETER_NAME: NameRule = NameRule {
    meta: &PARAMETER_NAME_META,
    what: "parameter",
    names: |node| {
        param_of(node, |owner| {
            matches!(owner.kind(), METHOD_DECL | CONSTRUCTOR_DECL)
        })
    },
};

static CATCH_PARAMETER_NAME_OPTIONS: [OptionSpec; 1] = format("^(e|t|ex|[a-z][a-z][a-zA-Z]+|_)$");
static CATCH_PARAMETER_NAME_META: RuleMeta = RuleMeta::new(
    "catch-parameter-name",
    "`catch` parameters should be named `e`, `t`, `ex`, or a camel-case word of three letters or more",
)
.checkstyle("CatchParameterName")
.opt_in()
.options(&CATCH_PARAMETER_NAME_OPTIONS);
// registry: catch-parameter-name => CATCH_PARAMETER_NAME
pub(crate) static CATCH_PARAMETER_NAME: NameRule = NameRule {
    meta: &CATCH_PARAMETER_NAME_META,
    what: "`catch` parameter",
    names: |node| param_of(node, |owner| owner.kind() == CATCH_CLAUSE),
};

static LAMBDA_PARAMETER_NAME_OPTIONS: [OptionSpec; 1] = format(CAMEL_OR_UNNAMED);
static LAMBDA_PARAMETER_NAME_META: RuleMeta = RuleMeta::new(
    "lambda-parameter-name",
    "lambda parameters should be named in camel case",
)
.checkstyle("LambdaParameterName")
.options(&LAMBDA_PARAMETER_NAME_OPTIONS);
// registry: lambda-parameter-name => LAMBDA_PARAMETER_NAME
pub(crate) static LAMBDA_PARAMETER_NAME: NameRule = NameRule {
    meta: &LAMBDA_PARAMETER_NAME_META,
    what: "lambda parameter",
    names: |node| param_of(node, |owner| owner.kind() == LAMBDA_EXPR),
};

static RECORD_COMPONENT_NAME_OPTIONS: [OptionSpec; 1] = format(CAMEL);
static RECORD_COMPONENT_NAME_META: RuleMeta = RuleMeta::new(
    "record-component-name",
    "record components should be named in camel case",
)
.checkstyle("RecordComponentName")
.options(&RECORD_COMPONENT_NAME_OPTIONS);
// registry: record-component-name => RECORD_COMPONENT_NAME
pub(crate) static RECORD_COMPONENT_NAME: NameRule = NameRule {
    meta: &RECORD_COMPONENT_NAME_META,
    what: "record component",
    names: |node| param_of(node, |owner| owner.kind() == RECORD_DECL),
};

static PATTERN_VARIABLE_NAME_OPTIONS: [OptionSpec; 1] = format(CAMEL_OR_UNNAMED);
static PATTERN_VARIABLE_NAME_META: RuleMeta = RuleMeta::new(
    "pattern-variable-name",
    "pattern variables should be named in camel case",
)
.checkstyle("PatternVariableName")
.options(&PATTERN_VARIABLE_NAME_OPTIONS);
// registry: pattern-variable-name => PATTERN_VARIABLE_NAME
pub(crate) static PATTERN_VARIABLE_NAME: NameRule = NameRule {
    meta: &PATTERN_VARIABLE_NAME_META,
    what: "pattern variable",
    names: |node| match node.kind() {
        TYPE_PATTERN => ast::name(node).into_iter().collect(),
        _ => Vec::new(),
    },
};

// --- Types and packages -------------------------------------------------------

static TYPE_NAME_OPTIONS: [OptionSpec; 1] = format("^[A-Z][a-zA-Z0-9]*$");
static TYPE_NAME_META: RuleMeta = RuleMeta::new(
    "type-name",
    "classes, interfaces, enums, records and annotation types should be named in upper camel case",
)
.checkstyle("TypeName")
.options(&TYPE_NAME_OPTIONS);
// registry: type-name => TYPE_NAME
pub(crate) static TYPE_NAME: NameRule = NameRule {
    meta: &TYPE_NAME_META,
    what: "type",
    names: |node| match node.kind().is_type_decl() {
        true => ast::name(node).into_iter().collect(),
        false => Vec::new(),
    },
};

/// The type parameters of `node`, if it is of kind `owner`.
fn type_params(node: &SyntaxNode, owner: fn(&SyntaxNode) -> bool) -> Vec<SyntaxToken> {
    if !owner(node) {
        return Vec::new();
    }
    child_node(node, TYPE_PARAMS)
        .map(|params| {
            params
                .children()
                .filter(|child| child.kind() == TYPE_PARAM)
                .filter_map(|param| ast::name(&param))
                .collect()
        })
        .unwrap_or_default()
}

static CLASS_TYPE_PARAMETER_NAME_OPTIONS: [OptionSpec; 1] = format(ONE_CAPITAL);
static CLASS_TYPE_PARAMETER_NAME_META: RuleMeta = RuleMeta::new(
    "class-type-parameter-name",
    "type parameters of classes should be a single capital letter",
)
.checkstyle("ClassTypeParameterName")
.opt_in()
.options(&CLASS_TYPE_PARAMETER_NAME_OPTIONS);
// registry: class-type-parameter-name => CLASS_TYPE_PARAMETER_NAME
pub(crate) static CLASS_TYPE_PARAMETER_NAME: NameRule = NameRule {
    meta: &CLASS_TYPE_PARAMETER_NAME_META,
    what: "type parameter",
    names: |node| type_params(node, |owner| owner.kind() == CLASS_DECL),
};

static INTERFACE_TYPE_PARAMETER_NAME_OPTIONS: [OptionSpec; 1] = format(ONE_CAPITAL);
static INTERFACE_TYPE_PARAMETER_NAME_META: RuleMeta = RuleMeta::new(
    "interface-type-parameter-name",
    "type parameters of interfaces should be a single capital letter",
)
.checkstyle("InterfaceTypeParameterName")
.opt_in()
.options(&INTERFACE_TYPE_PARAMETER_NAME_OPTIONS);
// registry: interface-type-parameter-name => INTERFACE_TYPE_PARAMETER_NAME
pub(crate) static INTERFACE_TYPE_PARAMETER_NAME: NameRule = NameRule {
    meta: &INTERFACE_TYPE_PARAMETER_NAME_META,
    what: "type parameter",
    names: |node| type_params(node, |owner| owner.kind() == INTERFACE_DECL),
};

static RECORD_TYPE_PARAMETER_NAME_OPTIONS: [OptionSpec; 1] = format(ONE_CAPITAL);
static RECORD_TYPE_PARAMETER_NAME_META: RuleMeta = RuleMeta::new(
    "record-type-parameter-name",
    "type parameters of records should be a single capital letter",
)
.checkstyle("RecordTypeParameterName")
.opt_in()
.options(&RECORD_TYPE_PARAMETER_NAME_OPTIONS);
// registry: record-type-parameter-name => RECORD_TYPE_PARAMETER_NAME
pub(crate) static RECORD_TYPE_PARAMETER_NAME: NameRule = NameRule {
    meta: &RECORD_TYPE_PARAMETER_NAME_META,
    what: "type parameter",
    names: |node| type_params(node, |owner| owner.kind() == RECORD_DECL),
};

static METHOD_TYPE_PARAMETER_NAME_OPTIONS: [OptionSpec; 1] = format(ONE_CAPITAL);
static METHOD_TYPE_PARAMETER_NAME_META: RuleMeta = RuleMeta::new(
    "method-type-parameter-name",
    "type parameters of methods should be a single capital letter",
)
.checkstyle("MethodTypeParameterName")
.opt_in()
.options(&METHOD_TYPE_PARAMETER_NAME_OPTIONS);
// registry: method-type-parameter-name => METHOD_TYPE_PARAMETER_NAME
pub(crate) static METHOD_TYPE_PARAMETER_NAME: NameRule = NameRule {
    meta: &METHOD_TYPE_PARAMETER_NAME_META,
    what: "type parameter",
    names: |node| type_params(node, |owner| owner.kind() == METHOD_DECL),
};

// --- Rules with more to them than a format ------------------------------------

/// Package names: lower case, with no underscores in the first part.
pub(crate) struct PackageName;

static PACKAGE_NAME_OPTIONS: [OptionSpec; 1] = format(r"^[a-z]+(\.[a-zA-Z_]\w*)*$");
static PACKAGE_NAME_META: RuleMeta =
    RuleMeta::new("package-name", "package names should be lower case")
        .checkstyle("PackageName")
        .options(&PACKAGE_NAME_OPTIONS);

impl Rule for PackageName {
    fn meta(&self) -> &'static RuleMeta {
        &PACKAGE_NAME_META
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if node.kind() != PACKAGE_DECL {
            return;
        }
        let Some(name) = child_node(node, NAME) else {
            return;
        };
        let text = ast::compact(&name);
        if !ctx.options.matches("format", &text) {
            out.push(Diagnostic::new(
                &PACKAGE_NAME_META,
                name.text_range(),
                format!("package name `{text}` should be lower case"),
            ));
        }
    }
}

/// Method names: camel case, and not the name of the class, which would
/// make the method look like a constructor.
pub(crate) struct MethodName;

static METHOD_NAME_OPTIONS: [OptionSpec; 2] = [
    OptionSpec {
        name: "format",
        default: OptionDefault::Regex(CAMEL),
        help: "what the name must match",
    },
    OptionSpec {
        name: "allow-class-name",
        default: OptionDefault::Bool(false),
        help: "allow a method to have the name of its class",
    },
];
static METHOD_NAME_META: RuleMeta = RuleMeta::new(
    "method-name",
    "methods should be named in camel case, and not after their class",
)
.checkstyle("MethodName")
.pmd("codestyle", "MethodNamingConventions")
.opt_in()
.options(&METHOD_NAME_OPTIONS);

impl Rule for MethodName {
    fn meta(&self) -> &'static RuleMeta {
        &METHOD_NAME_META
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        // An overriding method's name is not its author's to choose.
        if node.kind() != METHOD_DECL || ast::has_annotation(node, "Override") {
            return;
        }
        let Some(name) = ast::name(node) else {
            return;
        };
        if !ctx.options.matches("format", name.text()) {
            out.push(Diagnostic::new(
                &METHOD_NAME_META,
                name.text_range(),
                format!(
                    "method name `{}` should match `{}`",
                    name.text(),
                    ctx.options
                        .regex("format")
                        .map_or("", |regex| regex.as_str())
                ),
            ));
        }
        let class = ast::owner(node).filter(|owner| owner.kind().is_type_decl());
        if !ctx.options.bool("allow-class-name")
            && class.is_some_and(|class| ast::name_text(&class) == name.text())
        {
            out.push(Diagnostic::new(
                &METHOD_NAME_META,
                name.text_range(),
                format!("method `{}` has the name of its class", name.text()),
            ));
        }
    }
}

/// Runs of capitals in a camel-case name hide where its words begin:
/// `XMLHTTPRequest` against `XmlHttpRequest`.
pub(crate) struct AbbreviationAsWordInName;

static ABBREVIATION_OPTIONS: [OptionSpec; 6] = [
    OptionSpec {
        name: "allowed-abbreviation-length",
        default: OptionDefault::Int(3),
        help: "how many capitals may follow one another, not counting the one that starts the next word",
    },
    OptionSpec {
        name: "allowed-abbreviations",
        default: OptionDefault::List(&[]),
        help: "abbreviations that may be written in capitals whatever their length",
    },
    OptionSpec {
        name: "ignore-final",
        default: OptionDefault::Bool(true),
        help: "do not check final variables that are not static",
    },
    OptionSpec {
        name: "ignore-static",
        default: OptionDefault::Bool(true),
        help: "do not check static variables that are not final",
    },
    OptionSpec {
        name: "ignore-static-final",
        default: OptionDefault::Bool(true),
        help: "do not check constants",
    },
    OptionSpec {
        name: "ignore-overridden-methods",
        default: OptionDefault::Bool(true),
        help: "do not check methods annotated `@Override`",
    },
];
static ABBREVIATION_META: RuleMeta = RuleMeta::new(
    "abbreviation-as-word-in-name",
    "names should not contain long runs of capital letters",
)
.checkstyle("AbbreviationAsWordInName")
.opt_in()
.options(&ABBREVIATION_OPTIONS);

/// The first run of capitals in `name` that is longer than allowed. The
/// last capital of a run is taken to start the next word, so it does not
/// count; nor does the last character of the name.
fn long_abbreviation(name: &str, allowed: usize, exempt: &[String]) -> Option<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut start = None;
    for (i, c) in chars.iter().enumerate() {
        match (c.is_uppercase(), start) {
            (true, None) => start = Some(i),
            (false, Some(begin)) => {
                start = None;
                let end = i - 1;
                let run: String = chars[begin..end].iter().collect();
                if end - begin > allowed && !exempt.contains(&run) {
                    return Some(run);
                }
            }
            _ => {}
        }
    }
    let begin = start?;
    let end = chars.len() - 1;
    let run: String = chars[begin..end].iter().collect();
    (end - begin > allowed && !exempt.contains(&run)).then_some(run)
}

impl Rule for AbbreviationAsWordInName {
    fn meta(&self) -> &'static RuleMeta {
        &ABBREVIATION_META
    }

    fn check_node(&self, node: &SyntaxNode, ctx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let options = ctx.options;
        let names: Vec<SyntaxToken> = match node.kind() {
            FIELD_DECL | LOCAL_VAR_DECL => {
                let interface = node.kind() == FIELD_DECL && in_interface(node);
                let is_static = interface || ast::has_modifier(node, STATIC_KW);
                let is_final = interface || ast::has_modifier(node, FINAL_KW);
                let ignored = match (is_static, is_final) {
                    (true, true) => options.bool("ignore-static-final"),
                    (true, false) => options.bool("ignore-static"),
                    (false, true) => options.bool("ignore-final"),
                    (false, false) => false,
                };
                if ignored {
                    return;
                }
                declarators(node)
            }
            METHOD_DECL => {
                if options.bool("ignore-overridden-methods")
                    && ast::has_annotation(node, "Override")
                {
                    return;
                }
                ast::name(node).into_iter().collect()
            }
            PARAM if child_token(node, THIS_KW).is_none() => ast::name(node).into_iter().collect(),
            TYPE_PATTERN => ast::name(node).into_iter().collect(),
            kind if kind.is_type_decl() => ast::name(node).into_iter().collect(),
            _ => return,
        };
        let allowed = usize::try_from(options.int("allowed-abbreviation-length")).unwrap_or(0);
        for name in names {
            if long_abbreviation(name.text(), allowed, options.list("allowed-abbreviations"))
                .is_some()
            {
                out.push(Diagnostic::new(
                    &ABBREVIATION_META,
                    name.text_range(),
                    format!(
                        "`{}` has more than {} capital letters in a row",
                        name.text(),
                        allowed + 1
                    ),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::long_abbreviation;

    #[test]
    fn abbreviations() {
        let long = |name: &str| long_abbreviation(name, 3, &[]);
        assert_eq!(long("counterXYZ"), None);
        assert_eq!(long("customerID"), None);
        assert_eq!(long("XMLReader"), None);
        // Five capitals in a row: the last starts `Server`, four are left.
        assert_eq!(long("HTTPServer"), Some("HTTP".to_string()));
        assert_eq!(long("XMLHTTPRequest"), Some("XMLHTTP".to_string()));
        assert_eq!(long("counterABCDE"), Some("ABCD".to_string()));
        assert_eq!(long("CURRENT_COUNTER"), Some("CURREN".to_string()));
        assert_eq!(long("test_LANG_1311"), None);
        assert_eq!(
            long_abbreviation("XMLHTTPRequest", 3, &["XMLHTTP".to_string()]),
            None
        );
    }
}
