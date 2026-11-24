//! The lint rules. One module per rule, or per family of rules that share
//! their logic.
//!
//! To add a rule: create a module with a unit struct implementing
//! [`Rule`](crate::Rule) and a `static META`, then list it in [`ALL`].

mod array_type_style;
mod avoid_nested_blocks;
mod avoid_star_import;
mod avoid_static_import;
mod covariant_equals;
mod declaration_order;
mod default_comes_last;
mod design_for_extension;
mod empty_block;
mod empty_catch_block;
mod empty_statement;
mod equals_hash_code;
mod explicit_initialization;
mod fall_through;
mod file_length;
mod file_tab_character;
mod final_class;
mod final_parameters;
mod hidden_field;
mod hide_utility_class_constructor;
mod illegal_import;
mod inner_assignment;
mod interface_is_type;
mod line_length;
mod magic_number;
mod method_length;
mod missing_deprecated;
mod missing_override;
mod missing_switch_default;
mod modifier_order;
mod multiple_variable_declarations;
mod naming;
mod need_braces;
mod newline_at_end_of_file;
mod one_statement_per_line;
mod one_top_level_class;
mod outer_type_filename;
mod parameter_number;
mod redundant_import;
mod redundant_modifier;
mod regexp_singleline;
mod simplify_boolean_expression;
mod simplify_boolean_return;
mod string_literal_equality;
mod super_calls;
mod todo_comment;
mod uncommented_empty;
mod unused_imports;
mod upper_ell;
mod util;
mod visibility_modifier;

use crate::Rule;

/// Every rule, sorted by id.
pub(crate) static ALL: &[&dyn Rule] = &[
    &naming::AbbreviationAsWordInName,
    &array_type_style::ArrayTypeStyle,
    &avoid_nested_blocks::AvoidNestedBlocks,
    &avoid_star_import::AvoidStarImport,
    &avoid_static_import::AvoidStaticImport,
    &naming::CATCH_PARAMETER_NAME,
    &naming::CLASS_TYPE_PARAMETER_NAME,
    &naming::CONSTANT_NAME,
    &covariant_equals::CovariantEquals,
    &declaration_order::DeclarationOrder,
    &default_comes_last::DefaultComesLast,
    &design_for_extension::DesignForExtension,
    &empty_block::EmptyBlock,
    &empty_catch_block::EmptyCatchBlock,
    &empty_statement::EmptyStatement,
    &equals_hash_code::EqualsHashCode,
    &explicit_initialization::ExplicitInitialization,
    &fall_through::FallThrough,
    &file_length::FileLength,
    &file_tab_character::FileTabCharacter,
    &final_class::FinalClass,
    &final_parameters::FinalParameters,
    &hidden_field::HiddenField,
    &hide_utility_class_constructor::HideUtilityClassConstructor,
    &illegal_import::IllegalImport,
    &inner_assignment::InnerAssignment,
    &interface_is_type::InterfaceIsType,
    &naming::INTERFACE_TYPE_PARAMETER_NAME,
    &naming::LAMBDA_PARAMETER_NAME,
    &line_length::LineLength,
    &naming::LOCAL_FINAL_VARIABLE_NAME,
    &naming::LOCAL_VARIABLE_NAME,
    &magic_number::MagicNumber,
    &naming::MEMBER_NAME,
    &method_length::MethodLength,
    &naming::MethodName,
    &naming::METHOD_TYPE_PARAMETER_NAME,
    &missing_deprecated::MissingDeprecated,
    &missing_override::MissingOverride,
    &missing_switch_default::MissingSwitchDefault,
    &modifier_order::ModifierOrder,
    &multiple_variable_declarations::MultipleVariableDeclarations,
    &need_braces::NeedBraces,
    &newline_at_end_of_file::NewlineAtEndOfFile,
    &super_calls::NoClone,
    &super_calls::NoFinalizer,
    &one_statement_per_line::OneStatementPerLine,
    &one_top_level_class::OneTopLevelClass,
    &outer_type_filename::OuterTypeFilename,
    &naming::PackageName,
    &naming::PARAMETER_NAME,
    &parameter_number::ParameterNumber,
    &naming::PATTERN_VARIABLE_NAME,
    &naming::RECORD_COMPONENT_NAME,
    &naming::RECORD_TYPE_PARAMETER_NAME,
    &redundant_import::RedundantImport,
    &redundant_modifier::RedundantModifier,
    &regexp_singleline::RegexpSingleline,
    &simplify_boolean_expression::SimplifyBooleanExpression,
    &simplify_boolean_return::SimplifyBooleanReturn,
    &naming::STATIC_VARIABLE_NAME,
    &string_literal_equality::StringLiteralEquality,
    &super_calls::SuperClone,
    &super_calls::SuperFinalize,
    &todo_comment::TodoComment,
    &naming::TYPE_NAME,
    &uncommented_empty::UncommentedEmptyConstructor,
    &uncommented_empty::UncommentedEmptyMethodBody,
    &unused_imports::UnusedImports,
    &upper_ell::UpperEll,
    &visibility_modifier::VisibilityModifier,
];

#[cfg(test)]
mod tests {
    use crate::all_rules;

    #[test]
    fn rules_are_sorted_and_unique() {
        let ids: Vec<_> = all_rules().iter().map(|rule| rule.meta().id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn ids_are_kebab_case() {
        for rule in all_rules() {
            let id = rule.meta().id;
            assert!(
                id.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "{id}"
            );
            assert_ne!(id, crate::SYNTAX_ERROR);
        }
    }
}
