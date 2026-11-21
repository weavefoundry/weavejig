//! The kinds of tokens and nodes in a Java syntax tree.

macro_rules! syntax_kinds {
    (
        keywords { $($kw:ident = $kw_text:literal,)* }
        punctuation { $($p:ident = $p_text:literal,)* }
        tokens { $($t:ident,)* }
        nodes { $($n:ident,)* }
    ) => {
        /// Every kind of token and node. Tokens come first, then nodes.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[allow(non_camel_case_types, clippy::upper_case_acronyms)]
        #[repr(u16)]
        pub enum SyntaxKind {
            $($kw,)*
            $($p,)*
            $($t,)*
            $($n,)*
        }

        impl SyntaxKind {
            const ALL: &'static [SyntaxKind] = &[
                $(SyntaxKind::$kw,)*
                $(SyntaxKind::$p,)*
                $(SyntaxKind::$t,)*
                $(SyntaxKind::$n,)*
            ];

            /// The reserved word `text` names, if any. Contextual keywords
            /// (`var`, `record`, `sealed`, ...) are lexed as identifiers.
            pub fn from_keyword(text: &str) -> Option<SyntaxKind> {
                match text {
                    $($kw_text => Some(SyntaxKind::$kw),)*
                    _ => None,
                }
            }

            pub fn is_keyword(self) -> bool {
                matches!(self, $(SyntaxKind::$kw)|*)
            }

            pub fn is_punctuation(self) -> bool {
                matches!(self, $(SyntaxKind::$p)|*)
            }

            /// The source text of kinds that are always spelled the same way.
            pub fn static_text(self) -> Option<&'static str> {
                match self {
                    $(SyntaxKind::$kw => Some($kw_text),)*
                    $(SyntaxKind::$p => Some($p_text),)*
                    _ => None,
                }
            }
        }
    };
}

syntax_kinds! {
    keywords {
        ABSTRACT_KW = "abstract",
        ASSERT_KW = "assert",
        BOOLEAN_KW = "boolean",
        BREAK_KW = "break",
        BYTE_KW = "byte",
        CASE_KW = "case",
        CATCH_KW = "catch",
        CHAR_KW = "char",
        CLASS_KW = "class",
        CONST_KW = "const",
        CONTINUE_KW = "continue",
        DEFAULT_KW = "default",
        DO_KW = "do",
        DOUBLE_KW = "double",
        ELSE_KW = "else",
        ENUM_KW = "enum",
        EXTENDS_KW = "extends",
        FINAL_KW = "final",
        FINALLY_KW = "finally",
        FLOAT_KW = "float",
        FOR_KW = "for",
        GOTO_KW = "goto",
        IF_KW = "if",
        IMPLEMENTS_KW = "implements",
        IMPORT_KW = "import",
        INSTANCEOF_KW = "instanceof",
        INT_KW = "int",
        INTERFACE_KW = "interface",
        LONG_KW = "long",
        NATIVE_KW = "native",
        NEW_KW = "new",
        PACKAGE_KW = "package",
        PRIVATE_KW = "private",
        PROTECTED_KW = "protected",
        PUBLIC_KW = "public",
        RETURN_KW = "return",
        SHORT_KW = "short",
        STATIC_KW = "static",
        STRICTFP_KW = "strictfp",
        SUPER_KW = "super",
        SWITCH_KW = "switch",
        SYNCHRONIZED_KW = "synchronized",
        THIS_KW = "this",
        THROW_KW = "throw",
        THROWS_KW = "throws",
        TRANSIENT_KW = "transient",
        TRY_KW = "try",
        VOID_KW = "void",
        VOLATILE_KW = "volatile",
        WHILE_KW = "while",
        TRUE_KW = "true",
        FALSE_KW = "false",
        NULL_KW = "null",
    }
    punctuation {
        L_PAREN = "(",
        R_PAREN = ")",
        L_BRACE = "{",
        R_BRACE = "}",
        L_BRACK = "[",
        R_BRACK = "]",
        SEMI = ";",
        COMMA = ",",
        DOT = ".",
        ELLIPSIS = "...",
        AT = "@",
        COLON_COLON = "::",
        EQ = "=",
        // `>>` and `>>>` are never lexed as one token, so that `List<List<T>>`
        // needs no token splitting. The parser recognises shifts as adjacent
        // `>` tokens.
        GT = ">",
        LT = "<",
        BANG = "!",
        TILDE = "~",
        QUESTION = "?",
        COLON = ":",
        ARROW = "->",
        EQ_EQ = "==",
        LT_EQ = "<=",
        GT_EQ = ">=",
        BANG_EQ = "!=",
        AMP_AMP = "&&",
        PIPE_PIPE = "||",
        PLUS_PLUS = "++",
        MINUS_MINUS = "--",
        PLUS = "+",
        MINUS = "-",
        STAR = "*",
        SLASH = "/",
        AMP = "&",
        PIPE = "|",
        CARET = "^",
        PERCENT = "%",
        LT_LT = "<<",
        PLUS_EQ = "+=",
        MINUS_EQ = "-=",
        STAR_EQ = "*=",
        SLASH_EQ = "/=",
        AMP_EQ = "&=",
        PIPE_EQ = "|=",
        CARET_EQ = "^=",
        PERCENT_EQ = "%=",
        LT_LT_EQ = "<<=",
        GT_GT_EQ = ">>=",
        GT_GT_GT_EQ = ">>>=",
    }
    tokens {
        WHITESPACE,
        LINE_COMMENT,
        BLOCK_COMMENT,
        IDENT,
        INT_LITERAL,
        FLOAT_LITERAL,
        CHAR_LITERAL,
        STRING_LITERAL,
        TEXT_BLOCK,
        ERROR_TOKEN,
        EOF,
    }
    nodes {
        COMPILATION_UNIT,
        PACKAGE_DECL,
        IMPORT_DECL,
        MODULE_DECL,
        MODULE_DIRECTIVE,
        NAME,

        // Declarations
        CLASS_DECL,
        INTERFACE_DECL,
        ENUM_DECL,
        RECORD_DECL,
        ANNOTATION_TYPE_DECL,
        CLASS_BODY,
        ENUM_CONSTANT,
        FIELD_DECL,
        METHOD_DECL,
        CONSTRUCTOR_DECL,
        INITIALIZER_BLOCK,
        EMPTY_DECL,
        MODIFIERS,
        ANNOTATION,
        ANNOTATION_ARGS,
        ANNOTATION_PAIR,
        TYPE_PARAMS,
        TYPE_PARAM,
        TYPE_BOUND,
        EXTENDS_CLAUSE,
        IMPLEMENTS_CLAUSE,
        PERMITS_CLAUSE,
        THROWS_CLAUSE,
        PARAM_LIST,
        PARAM,
        DEFAULT_VALUE,
        VARIABLE_DECLARATOR,

        // Types
        PRIMITIVE_TYPE,
        NAMED_TYPE,
        ARRAY_TYPE,
        WILDCARD_TYPE,
        INTERSECTION_TYPE,
        UNION_TYPE,
        TYPE_ARGS,

        // Statements
        BLOCK,
        LOCAL_VAR_DECL,
        EXPR_STMT,
        IF_STMT,
        FOR_STMT,
        FOREACH_STMT,
        WHILE_STMT,
        DO_STMT,
        TRY_STMT,
        RESOURCE_LIST,
        RESOURCE,
        CATCH_CLAUSE,
        FINALLY_CLAUSE,
        SWITCH_STMT,
        SWITCH_BLOCK,
        SWITCH_GROUP,
        SWITCH_RULE,
        CASE_LABEL,
        GUARD,
        SYNCHRONIZED_STMT,
        RETURN_STMT,
        THROW_STMT,
        BREAK_STMT,
        CONTINUE_STMT,
        YIELD_STMT,
        ASSERT_STMT,
        LABELED_STMT,
        EMPTY_STMT,

        // Expressions
        LITERAL,
        NAME_EXPR,
        FIELD_ACCESS,
        METHOD_CALL,
        ARG_LIST,
        ARRAY_ACCESS,
        THIS_EXPR,
        SUPER_EXPR,
        PAREN_EXPR,
        PREFIX_EXPR,
        POSTFIX_EXPR,
        BINARY_EXPR,
        INSTANCEOF_EXPR,
        ASSIGN_EXPR,
        CONDITIONAL_EXPR,
        CAST_EXPR,
        LAMBDA_EXPR,
        LAMBDA_PARAMS,
        METHOD_REF,
        NEW_EXPR,
        ARRAY_CREATION_EXPR,
        DIM_EXPR,
        ARRAY_INIT,
        CLASS_LITERAL,
        SWITCH_EXPR,
        TYPE_PATTERN,
        RECORD_PATTERN,

        ERROR,
    }
}

use SyntaxKind::*;

impl SyntaxKind {
    pub fn is_trivia(self) -> bool {
        matches!(self, WHITESPACE | LINE_COMMENT | BLOCK_COMMENT)
    }

    pub fn is_comment(self) -> bool {
        matches!(self, LINE_COMMENT | BLOCK_COMMENT)
    }

    pub fn is_token(self) -> bool {
        (self as u16) <= (EOF as u16)
    }

    pub fn is_literal(self) -> bool {
        matches!(
            self,
            INT_LITERAL
                | FLOAT_LITERAL
                | CHAR_LITERAL
                | STRING_LITERAL
                | TEXT_BLOCK
                | TRUE_KW
                | FALSE_KW
                | NULL_KW
        )
    }

    pub fn is_primitive_type(self) -> bool {
        matches!(
            self,
            BOOLEAN_KW | BYTE_KW | SHORT_KW | INT_KW | LONG_KW | CHAR_KW | FLOAT_KW | DOUBLE_KW
        )
    }

    /// Keywords that can appear in a modifier list. `sealed` and `non-sealed`
    /// are contextual and handled by the parser.
    pub fn is_modifier_keyword(self) -> bool {
        matches!(
            self,
            PUBLIC_KW
                | PROTECTED_KW
                | PRIVATE_KW
                | ABSTRACT_KW
                | STATIC_KW
                | FINAL_KW
                | TRANSIENT_KW
                | VOLATILE_KW
                | SYNCHRONIZED_KW
                | NATIVE_KW
                | STRICTFP_KW
                | DEFAULT_KW
        )
    }

    pub fn is_assignment_op(self) -> bool {
        matches!(
            self,
            EQ | PLUS_EQ
                | MINUS_EQ
                | STAR_EQ
                | SLASH_EQ
                | AMP_EQ
                | PIPE_EQ
                | CARET_EQ
                | PERCENT_EQ
                | LT_LT_EQ
                | GT_GT_EQ
                | GT_GT_GT_EQ
        )
    }

    pub fn is_type_decl(self) -> bool {
        matches!(
            self,
            CLASS_DECL | INTERFACE_DECL | ENUM_DECL | RECORD_DECL | ANNOTATION_TYPE_DECL
        )
    }

    pub(crate) fn from_raw(raw: u16) -> SyntaxKind {
        Self::ALL[raw as usize]
    }
}

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        rowan::SyntaxKind(kind as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_round_trip() {
        for (i, kind) in SyntaxKind::ALL.iter().enumerate() {
            assert_eq!(*kind as usize, i);
            assert_eq!(SyntaxKind::from_raw(i as u16), *kind);
        }
    }

    #[test]
    fn keywords() {
        assert_eq!(SyntaxKind::from_keyword("class"), Some(CLASS_KW));
        assert_eq!(SyntaxKind::from_keyword("record"), None);
        assert_eq!(GT_GT_GT_EQ.static_text(), Some(">>>="));
        assert!(EOF.is_token() && !COMPILATION_UNIT.is_token());
    }
}
