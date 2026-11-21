//! The lint rules. One module per rule.
//!
//! To add a rule: create a module with a unit struct implementing
//! [`Rule`](crate::Rule) and a `static META`, then list it in [`ALL`].

mod array_type_style;
mod avoid_star_import;
mod empty_statement;
mod modifier_order;
mod need_braces;
mod upper_ell;

use crate::Rule;

/// Every rule, sorted by id.
pub(crate) static ALL: &[&dyn Rule] = &[
    &array_type_style::ArrayTypeStyle,
    &avoid_star_import::AvoidStarImport,
    &empty_statement::EmptyStatement,
    &modifier_order::ModifierOrder,
    &need_braces::NeedBraces,
    &upper_ell::UpperEll,
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
