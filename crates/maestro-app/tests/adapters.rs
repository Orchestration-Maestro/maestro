//! Selecting an adapter by name through the registry.
#![cfg(test)]

use std::sync::Arc;

use maestro_app::adapters::{AdapterError, Installed, Port, Registry};
use maestro_settings::{BaseSettingsLayer, Settings, SettingsManager, SettingsScope};
use serde_json::json;

/// The operations a test port needs of its adapters.
trait Greeter: Send + Sync {
    /// Names the double that answers.
    fn greet(&self) -> String;
}

/// A double that answers with its own name.
struct Named(&'static str);

impl Greeter for Named {
    fn greet(&self) -> String {
        self.0.to_owned()
    }
}

/// The test port; `first` is its default adapter.
const GREETER: Port<dyn Greeter> = Port::new("greeter", "first");

/// A registry holding two doubles under the test port.
fn registry() -> Registry {
    let mut registry = Registry::default();
    for name in ["first", "second"] {
        registry
            .register(&GREETER, name, move || {
                Arc::new(Named(name)) as Arc<dyn Greeter>
            })
            .expect("a complete double registers");
    }
    registry
}

/// A settings document holding the `adapters` object.
fn document(adapters: &serde_json::Value) -> Settings {
    Settings(
        json!({ "adapters": adapters })
            .as_object()
            .cloned()
            .unwrap_or_default(),
    )
}

/// A manager whose global settings select adapters.
fn selecting(adapters: &serde_json::Value) -> SettingsManager {
    SettingsManager::in_memory(document(adapters))
}

#[test]
fn each_registered_double_is_selected_by_its_setting() {
    let registry = registry();
    for name in ["first", "second"] {
        let settings = selecting(&json!({ "greeter": name }));
        let adapter = registry.select(&GREETER, &settings).expect("selected");
        assert_eq!(adapter.greet(), name);
    }
}

#[test]
fn an_unknown_name_is_refused_with_the_installed_names() {
    let settings = selecting(&json!({ "greeter": "third" }));
    let Err(error) = registry().select(&GREETER, &settings) else {
        panic!("an unknown name must be refused");
    };
    assert_eq!(
        error,
        AdapterError::Unknown {
            port: "greeter",
            name: "third".into(),
            installed: vec!["first".into(), "second".into()],
        }
    );
    assert_eq!(
        error.to_string(),
        "no adapter `third` for port `greeter`; installed: first, second"
    );
}

#[test]
fn a_port_without_a_setting_selects_its_default_adapter() {
    let registry = registry();
    let adapter = registry
        .select(&GREETER, &selecting(&json!({ "other": "second" })))
        .expect("the default is selected");
    assert_eq!(adapter.greet(), "first");
}

#[test]
fn a_malformed_adapter_setting_is_refused_instead_of_falling_back() {
    let Err(error) = registry().select(&GREETER, &selecting(&json!({ "greeter": 7 }))) else {
        panic!("a non-string name must be refused");
    };
    assert_eq!(error.to_string(), "Invalid adapters.greeter setting: 7");
}

#[test]
fn an_unregistered_default_is_refused_with_the_installed_names() {
    let port: Port<dyn Greeter> = Port::new("greeter", "absent");
    let Err(error) = registry().select(&port, &selecting(&json!({}))) else {
        panic!("an unregistered default must be refused");
    };
    assert_eq!(
        error.to_string(),
        "no adapter `absent` for port `greeter`; installed: first, second"
    );
}

#[test]
fn registering_the_same_name_twice_is_refused_and_keeps_the_first() {
    let mut registry = registry();
    let error = registry
        .register(&GREETER, "first", || {
            Arc::new(Named("impostor")) as Arc<dyn Greeter>
        })
        .expect_err("a second `first` is refused");
    assert_eq!(
        error,
        AdapterError::Duplicate {
            port: "greeter",
            name: "first".into(),
        }
    );
    let settings = selecting(&json!({ "greeter": "first" }));
    let adapter = registry.select(&GREETER, &settings).expect("selected");
    assert_eq!(adapter.greet(), "first");
}

#[test]
fn installed_reports_every_registration_in_order() {
    let mut registry = registry();
    registry
        .register(&OTHER, "only", || {
            Arc::new(Named("only")) as Arc<dyn Greeter>
        })
        .expect("registers");
    let report = registry.installed();
    let pairs: Vec<(&str, &str)> = report
        .iter()
        .map(|Installed { port, name }| (*port, name.as_str()))
        .collect();
    assert_eq!(
        pairs,
        [
            ("greeter", "first"),
            ("greeter", "second"),
            ("other", "only")
        ]
    );
}

/// A second port.
const OTHER: Port<dyn Greeter> = Port::new("other", "only");

#[test]
fn a_locked_adapter_key_selects_the_base_name() {
    let mut settings = selecting(&json!({ "greeter": "second" }));
    settings.publish_base_layer(
        SettingsScope::Global,
        Some(BaseSettingsLayer {
            settings: document(&json!({ "greeter": "first" })),
            locked: ["adapters.greeter".to_owned()].into(),
            source: "git:example/config#v1".into(),
            recorded_at: "2026-10-27T12:00:00Z".into(),
        }),
    );
    let adapter = registry().select(&GREETER, &settings).expect("selected");
    assert_eq!(adapter.greet(), "first");
}
