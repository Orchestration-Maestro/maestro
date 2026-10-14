//! Public native factory selection and delegated caller behavior.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

#[test]
fn maestro_tool_presets_preserve_order_and_membership() {
    crate::factory_support::check_order();
}

#[test]
fn maestro_factory_routes_only_its_tool_options() {
    crate::factory_support::check_metadata();
    crate::factory_support::check_routes();
}

#[test]
fn maestro_aggregate_tools_use_native_and_controlled_operations() {
    crate::factory_support::check_operations();
}

#[test]
fn maestro_aggregate_edit_prepares_before_validation() {
    crate::factory_support::check_preparation();
}

#[test]
fn maestro_factories_expose_corrected_write_and_search_results() {
    crate::factory_support::check_corrections();
}
