//! Persisted global preferences through the daemon configuration boundary.

use crate::settings::test_support::layer;
use crate::settings::{io::load_config, tabs::TabBarRightEntryConfig};
use crate::tests::block_on;
use maestro_settings::{InMemorySettingsStorage, SettingsManager, SettingsScope, SettingsStorage};
use serde_json::json;
use std::{path::Path, sync::Arc};
#[cfg(target_os = "linux")]
#[test]
fn daemon_invalid_area_retains_unlocked_values_through_lock_changes() {
    use crate::state::tests::config_support::{block_on, fixture, put};
    let (mut app, mut manager, storage) = fixture();
    put(
        &*storage,
        r#"{"multiplexer":{"mouseCapture":false,"tabBarRight":[]}}"#,
    );
    block_on(app.reload_config(&mut manager, Path::new(".")));
    for entries in [json!([{"type":"hostname"}]), json!([])] {
        storage
            .write_base_layer(
                SettingsScope::Global,
                Some(
                    &layer(
                        json!({"multiplexer":{"tabBarRight":entries}}),
                        &["multiplexer.tabBarRight"],
                    )
                    .unwrap(),
                ),
            )
            .unwrap();
        put(
            &*storage,
            r#"{"multiplexer":{"sidebarWidth":"bad","mouseCapture":true}}"#,
        );
        let (report, errors) = block_on(app.reload_config(&mut manager, Path::new(".")));
        assert!(errors.is_empty());
        assert!(!app.config.ui.mouse_capture.0);
        assert_eq!(
            serde_json::to_value(&app.config.ui.tab_bar_right).unwrap(),
            entries
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|message| message.starts_with("invalid ui config:"))
        );
    }
}
#[cfg(target_os = "linux")]
#[test]
fn daemon_first_reload_retains_startup_area_document() {
    use crate::state::tests::config_support::put;
    use crate::{service::event_hub::EventHub, state::App};
    let storage = Arc::new(InMemorySettingsStorage::new());
    put(&*storage, r#"{"multiplexer":{"mouseCapture":false}}"#);
    let mut manager = SettingsManager::from_storage(storage.clone());
    let loaded = load_config(&mut manager, Path::new(".")).0;
    let manifests = Arc::new(std::sync::RwLock::new(
        maestro_detection::definitions::ManifestCache::new(None, None).unwrap(),
    ));
    let (launch, receiver) =
        crate::state::test_support::spawning::launch_inputs(&[], manifests).unwrap();
    let mut app = App::from_loaded(&loaded, || 0, launch, receiver, EventHub::default());
    storage
        .write_base_layer(
            SettingsScope::Global,
            Some(
                &layer(
                    json!({"multiplexer":{"tabBarRight":[{"type":"hostname"}]}}),
                    &["multiplexer.tabBarRight"],
                )
                .unwrap(),
            ),
        )
        .unwrap();
    put(&*storage, r#"{"multiplexer":{"sidebarWidth":"bad"}}"#);
    block_on(app.reload_config(&mut manager, Path::new(".")));
    assert!(!app.config.ui.mouse_capture.0);
    assert_eq!(
        app.config.ui.tab_bar_right,
        vec![TabBarRightEntryConfig::Hostname {}]
    );
}
#[cfg(target_os = "linux")]
#[test]
fn daemon_runtime_rejected_documents_do_not_replace_accepted_areas() {
    use crate::state::tests::config_support::{fixture, put};
    let (mut app, mut manager, storage) = fixture();
    put(
        &*storage,
        r#"{"multiplexer":{"mouseCapture":false},"daemon":{"headlessCols":100}}"#,
    );
    block_on(app.reload_config(&mut manager, Path::new(".")));
    put(
        &*storage,
        r#"{"multiplexer":{"sidebarMinWidth":50,"sidebarMaxWidth":30},"daemon":{"headlessCols":0}}"#,
    );
    block_on(app.reload_config(&mut manager, Path::new(".")));
    storage.write_base_layer(SettingsScope::Global, Some(&layer(
        json!({"multiplexer":{"tabBarRight":[{"type":"hostname"}]},"daemon":{"headlessRows":47}}),
        &["multiplexer.tabBarRight", "daemon.headlessRows"],
    ).unwrap())).unwrap();
    put(
        &*storage,
        r#"{"multiplexer":{"sidebarWidth":"bad"},"daemon":{"headlessCols":"bad"}}"#,
    );
    let (report, errors) = block_on(app.reload_config(&mut manager, Path::new(".")));
    assert!(errors.is_empty());
    assert!(!report.diagnostics.is_empty());
    assert!(!app.config.ui.mouse_capture.0);
    assert_eq!(
        app.config.ui.tab_bar_right,
        vec![TabBarRightEntryConfig::Hostname {}]
    );
    assert_eq!(app.config.server.headless_cols, 100);
    assert_eq!(app.config.server.headless_rows, 47);
}
#[cfg(target_os = "linux")]
#[test]
fn daemon_no_layer_rejected_ui_does_not_reset_scroll() {
    use crate::state::tests::config_support::{fixture, put};
    let (mut app, mut manager, storage) = fixture();
    put(&*storage, "{}");
    block_on(app.reload_config(&mut manager, Path::new(".")));
    app.state.agent_panel_scroll = 7;
    put(&*storage, r#"{"multiplexer":{"sidebarWidth":"bad"}}"#);
    block_on(app.reload_config(&mut manager, Path::new(".")));
    assert_eq!(app.state.agent_panel_scroll, 7);
}
#[cfg(target_os = "linux")]
#[test]
fn daemon_typed_constructor_retains_ui_on_first_invalid_no_layer_reload() {
    use crate::state::tests::config_support::{fixture, put};
    let (mut app, mut manager, storage) = fixture();
    app.config.ui.mouse_capture.0 = false;
    put(&*storage, r#"{"multiplexer":{"sidebarWidth":"bad"}}"#);
    block_on(app.reload_config(&mut manager, Path::new(".")));
    assert!(!app.config.ui.mouse_capture.0);
}
