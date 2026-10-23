//! Loading extensions from paths and discovering them in standard and configured locations.
#![cfg(test)]
#![cfg(unix)]
use crate::module_loader::{Trace, check, check_with_home, check_with_home_dot, module_loader};
use serde_json::json;

#[test]
fn explicit_paths_load_in_order_without_discovery() {
    check(&[
        "load_empty_list",
        "load_only_explicit_paths",
        "load_in_given_order",
    ]);
}

#[test]
fn failed_paths_record_messages_and_loading_continues() {
    check(&[
        "load_without_factory",
        "load_module_failure",
        "load_factory_failure",
        "load_continues_after_failures",
    ]);
}

#[test]
fn loaded_paths_keep_original_and_resolved_spellings() {
    check(&[
        "load_relative_paths",
        "load_absolute_spelling_kept",
        "load_unicode_spaces",
    ]);
}

#[test]
fn home_paths_expand_against_the_home_directory() {
    check_with_home(
        "home_paths_expand_against_the_home_directory",
        &["load_home_paths", "discover_configured_files"],
    );
}

#[test]
fn bare_home_normalizes_the_home_spelling_before_loading() {
    check_with_home_dot(
        "bare_home_normalizes_the_home_spelling_before_loading",
        &["load_home_dot_spelling"],
    );
}

#[test]
fn loaded_paths_share_the_bus_and_runtime_of_one_call() {
    check(&[
        "load_supplied_bus",
        "load_shares_bus_within_call",
        "load_fresh_bus_per_call",
        "load_shares_runtime",
        "discover_supplied_bus",
    ]);
}

/// A foreign listener reached from inside a callback runs after that callback returns,
/// so each loaded path received its own bus identity.
#[test]
fn each_loaded_path_has_its_own_bus_identity() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path().display().to_string();
    std::fs::write(format!("{root}/relay.ts"), "relay:a>b").unwrap();
    std::fs::write(format!("{root}/sink.ts"), "listen:b").unwrap();
    crate::local_support::local(async {
        let trace = Trace::default();
        let bus = crate::local_support::LocalBus::new();
        let paths = vec![format!("{root}/relay.ts"), format!("{root}/sink.ts")];
        let result = maestro_extensions::load_extensions(
            paths,
            &root,
            &bus.controller,
            &module_loader(&trace),
        )
        .await
        .unwrap();
        assert_eq!(result.extensions.len(), 2);
        bus.controller.emit("a", json!({})).await;
        bus.settle().await;
        let sink = json!({"listener": format!("{root}/sink.ts"), "channel": "b", "data": {}});
        assert_eq!(*trace.borrow(), [json!("relay returned"), sink]);
    });
}

/// The default family runs path loading and discovery as `Send` tasks.
#[test]
fn default_family_loads_through_a_thread_safe_loader() {
    use maestro_extensions::{
        ExtensionFactory, ModuleLoader, create_event_bus, discover_and_load_extensions,
        load_extensions, types::RegisteredCommandOptions,
    };
    use std::sync::Arc;
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path().display().to_string();
    let factory: ExtensionFactory = Arc::new(|api| {
        Box::pin(async move {
            api.register_command(
                "hello".into(),
                RegisteredCommandOptions {
                    description: None,
                    get_argument_completions: None,
                    handler: Arc::new(|_, _| Box::pin(async { Ok(()) })),
                },
            )
        })
    });
    let loader: ModuleLoader =
        Arc::new(move |_| Box::pin(std::future::ready(Ok(Some(factory.clone())))));
    let bus = create_event_bus(Arc::new(drop), Arc::new(|_| {}));
    let runtime = crate::runtime_support::runtime();
    let (explicit, discovered) = runtime.block_on(async move {
        let paths = vec!["ext.ts".to_owned()];
        let (cwd, task_bus, task_loader) = (root.clone(), bus.clone(), loader.clone());
        let load =
            tokio::spawn(
                async move { load_extensions(paths, &cwd, &task_bus, &task_loader).await },
            );
        let configured = vec!["found.ts".to_owned()];
        let discover = tokio::spawn(async move {
            discover_and_load_extensions(&configured, &root, "/absent", &bus, &loader).await
        });
        (
            load.await.unwrap().unwrap(),
            discover.await.unwrap().unwrap(),
        )
    });
    for (result, name) in [(explicit, "ext.ts"), (discovered, "found.ts")] {
        assert!(result.errors.is_empty());
        let [extension] = result.extensions.as_slice() else {
            panic!("one extension expected");
        };
        let extension = extension.read().unwrap();
        assert_eq!(
            extension.resolved_path,
            format!("{}/{name}", scratch.path().display())
        );
        assert!(extension.commands.contains_key("hello"));
    }
}

#[test]
fn discovery_admits_files_indexes_and_manifest_entries() {
    check(&[
        "discover_direct_files",
        "discover_index_files",
        "discover_manifest_entries",
        "discover_registers_each_kind",
        "discover_ignores_plain_subdirectory",
        "discover_one_level_only",
        "discover_mixed_entries",
    ]);
}

#[test]
fn discovery_orders_locations_and_keeps_first_spellings() {
    check(&["discover_location_order", "discover_first_wins"]);
}

#[test]
fn configured_paths_load_files_and_directories() {
    check(&[
        "discover_configured_directories",
        "discover_configured_missing",
    ]);
}

#[test]
fn discovery_follows_links_and_skips_unlistable_directories() {
    check(&[
        "discover_symlinks",
        "discover_missing_directories",
        "discover_unreadable_directory",
    ]);
}

#[test]
fn manifest_item_errors_drop_a_directory_or_reject_a_configured_one() {
    check(&[
        "discover_bad_manifest_in_subdirectory",
        "discover_bad_manifest_in_configured_directory",
    ]);
}

#[test]
fn existing_files_keep_their_real_spelling() {
    check(&[
        "load_unicode_space_in_disk_name",
        "discover_unicode_space_in_discovered_name",
    ]);
}
