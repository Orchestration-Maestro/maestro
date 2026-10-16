//! Self-update command selection through supplied observations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

#[path = "support/self_update/mod.rs"]
mod support;

#[test]
fn update_commands_cover_each_install_method() {
    support::run(
        include_str!(
            "fixtures/configuration/self_update/update_commands_cover_each_install_method.json"
        ),
        false,
    );
}

#[test]
fn renamed_packages_keep_ordered_uninstall_steps() {
    support::run(
        include_str!(
            "fixtures/configuration/self_update/renamed_packages_keep_ordered_uninstall_steps.json"
        ),
        false,
    );
}

#[test]
fn command_displays_use_shared_whitespace_semantics() {
    support::run(
        include_str!(
            "fixtures/configuration/self_update/command_displays_use_shared_whitespace_semantics.json"
        ),
        false,
    );
}

#[test]
fn configuration_roots_flow_into_update_selection() {
    support::run(
        include_str!(
            "fixtures/configuration/self_update/configuration_roots_flow_into_update_selection.json"
        ),
        true,
    );
}

#[test]
fn configured_bun_uses_both_global_roots() {
    support::run(
        include_str!(
            "fixtures/configuration/self_update/configured_bun_uses_both_global_roots.json"
        ),
        false,
    );
}

#[test]
fn configured_npm_commands_control_queries_and_arguments() {
    support::run(
        include_str!(
            "fixtures/configuration/self_update/configured_npm_commands_control_queries_and_arguments.json"
        ),
        false,
    );
}

#[test]
fn global_roots_follow_each_manager() {
    support::run(
        include_str!("fixtures/configuration/self_update/global_roots_follow_each_manager.json"),
        false,
    );
}

#[test]
fn inferred_prefixes_require_unix_global_layout() {
    support::run(
        include_str!(
            "fixtures/configuration/self_update/inferred_prefixes_require_unix_global_layout.json"
        ),
        false,
    );
}

#[test]
fn instructions_preserve_install_guidance() {
    support::run(
        include_str!(
            "fixtures/configuration/self_update/instructions_preserve_install_guidance.json"
        ),
        false,
    );
}

#[test]
fn output_probes_preserve_trim_and_failure_precedence() {
    support::run(
        include_str!(
            "fixtures/configuration/self_update/output_probes_preserve_trim_and_failure_precedence.json"
        ),
        false,
    );
}

#[test]
fn root_evidence_requires_canonical_descendants() {
    support::run(
        include_str!(
            "fixtures/configuration/self_update/root_evidence_requires_canonical_descendants.json"
        ),
        false,
    );
}

#[test]
fn selector_reads_observations_only_on_reachable_branches() {
    support::run(
        include_str!(
            "fixtures/configuration/self_update/selector_reads_observations_only_on_reachable_branches.json"
        ),
        false,
    );
}

#[test]
fn self_update_records_outlive_selection_inputs() {
    support::run_owned(include_str!(
        "fixtures/configuration/self_update/self_update_records_outlive_selection_inputs.json"
    ));
}

#[test]
fn writable_checks_gate_automatic_updates() {
    support::run(
        include_str!(
            "fixtures/configuration/self_update/writable_checks_gate_automatic_updates.json"
        ),
        false,
    );
}
