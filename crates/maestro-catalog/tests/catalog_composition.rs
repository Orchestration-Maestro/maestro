#![cfg(test)]
//! Descriptor defaults and ordered composition.
mod support;
#[test]
fn file_models_keep_defaults_and_field_precedence() {
    support::corpus("file_models_keep_defaults_and_field_precedence");
}
#[test]
fn overrides_change_only_selected_builtin_fields() {
    support::corpus("overrides_change_only_selected_builtin_fields");
}

#[test]
fn builtin_custom_models_inherit_first_descriptor_only() {
    support::corpus("builtin_custom_models_inherit_first_descriptor_only");
}

#[test]
fn provider_overrides_preserve_inventory_and_isolate_other_providers() {
    support::corpus("provider_overrides_preserve_inventory_and_isolate_other_providers");
}

#[test]
fn custom_replacements_keep_first_position_and_last_value() {
    support::corpus("custom_replacements_keep_first_position_and_last_value");
}

#[test]
fn unconsumed_typed_members_do_not_reject_valid_models() {
    support::corpus("unconsumed_typed_members_do_not_reject_valid_models");
}

#[test]
fn routing_merges_one_level_without_reordering_lists() {
    support::corpus("routing_merges_one_level_without_reordering_lists");
}

#[test]
fn thinking_overrides_preserve_nulls_and_other_levels() {
    support::corpus("thinking_overrides_preserve_nulls_and_other_levels");
}
