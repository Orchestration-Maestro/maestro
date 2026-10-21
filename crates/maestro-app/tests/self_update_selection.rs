//! Self-update command selection through supplied observations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use maestro_test_process::{TestProcess, test_name};

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

/// Only a Unix process can end without an exit code (by signal).
#[cfg(unix)]
#[test]
fn signal_only_probe_status_reports_unknown_exit_code() {
    support::run(
        include_str!(
            "fixtures/configuration/self_update/signal_only_probe_status_reports_unknown_exit_code.json"
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

#[test]
fn supplied_path_flavors_reach_update_evidence() {
    support::run(
        include_str!(
            "fixtures/configuration/self_update/supplied_path_flavors_reach_update_evidence.json"
        ),
        false,
    );
}

#[test]
#[cfg(unix)]
fn native_queries_preserve_literal_executable_and_arguments() {
    literal_query();
}

#[test]
#[cfg(windows)]
fn windows_native_queries_preserve_literal_executable_and_arguments() {
    literal_query();
}

/// Runs the self-update query and the package-manager command against a real executable
/// under a spaced path.
fn literal_query() {
    use maestro_app::configuration::config::{NativeSelfUpdateOperations, SelfUpdateOperations};
    use maestro_packages::{NativePackageOperations, PackageOperations};
    let packages = NativePackageOperations::new(
        std::rc::Rc::new(|| false),
        &std::rc::Rc::new(tokio::task::LocalSet::new()),
    );
    let child = || {
        TestProcess::named(test_name(native_query_child).unwrap())
            .unwrap()
            .nocapture()
            .skip("argument with spaces")
            .skip("literal & | ; $HOME %PATH% ^ < >")
    };
    let directory = std::env::temp_dir().join(format!("maestro query {}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let executable = directory.join(format!("npm{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
    // Omitting the Windows suffix exercises the package-manager command spelling.
    let commands = vec![directory.join("npm")];
    #[cfg(windows)]
    let commands = {
        let mut commands = commands;
        std::fs::write(directory.join("custom.cmd"), "@\"%~dp0npm.exe\" %*\r\n").unwrap();
        commands.extend([directory.join("custom.cmd"), directory.join("custom")]);
        commands
    };
    for command in commands {
        let command = command.to_str().unwrap();
        let output = child()
            .output_with(|args| NativeSelfUpdateOperations.run_command_sync(command, args))
            .unwrap();
        assert!(output.status.success(), "{command:?}: {output:?}");
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains("literal argv received")
        );
        // The child prints its marker only after checking that every argument arrived intact.
        let package = child()
            .invoke_with(|args| packages.run_command_sync(command, args))
            .unwrap();
        assert_eq!(package.status, Some(0), "{command:?}: {}", package.stderr);
        assert!(
            package.stdout.contains("literal argv received"),
            "{command:?}: {}",
            package.stdout
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn native_query_child() {
    let args: Vec<_> = std::env::args().collect();
    if std::path::Path::new(&args[0])
        .file_stem()
        .is_none_or(|name| name != "npm")
    {
        return;
    }
    assert_eq!(
        &args[4..],
        [
            "--skip",
            "argument with spaces",
            "--skip",
            "literal & | ; $HOME %PATH% ^ < >"
        ]
    );
    println!("literal argv received");
}
