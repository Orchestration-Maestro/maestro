//! Ordered model files share one parser and retain their underlying layers on refresh.
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_catalog::{ModelRegistry, NativeModelFileOperations, ProviderConfigInput};
use maestro_test_directory::Scratch;
use std::sync::{Arc, RwLock};

/// These literals distinguish partial overrides from whole custom-model replacement.
fn assert_layers(registry: &ModelRegistry) {
    assert_eq!(registry.get_error(), None);
    let model = registry.find("openai", "gpt-4o").unwrap();
    let model = model.read().unwrap();
    assert_eq!(model.context_window.to_bits(), 12345.0_f64.to_bits());
    assert_eq!(model.cost.input.to_bits(), 3.0_f64.to_bits());
    assert_eq!(model.cost.output.to_bits(), 2.0_f64.to_bits());
    let retained = registry.find("layered", "retained").unwrap();
    let retained = retained.read().unwrap();
    assert_eq!(retained.name, "Base retained");
    assert_eq!(retained.max_tokens.to_bits(), 777.0_f64.to_bits());
    assert_eq!(retained.base_url, "https://base.invalid/v1");
    assert_eq!(
        retained.compat.as_ref().unwrap().0,
        serde_json::json!({"openRouterRouting":{"order":["base"],"max_price":{"prompt":3}}})
            .as_object()
            .unwrap()
            .clone()
    );
    let replaced = registry.find("layered", "replaced").unwrap();
    let replaced = replaced.read().unwrap();
    assert_eq!(replaced.name, "User replaced");
    assert_eq!(replaced.context_window.to_bits(), 128_000.0_f64.to_bits());
}

/// Refresh and removal of a dynamic override restore the same composed file layers.
#[test]
fn ordered_model_files_preserve_partial_overrides_and_replace_custom_definitions() {
    if crate::isolation::isolated(
        "ordered_model_files_preserve_partial_overrides_and_replace_custom_definitions",
    ) {
        return;
    }
    let scratch = Scratch::new().unwrap();
    let mut paths = Vec::new();
    for (name, text) in [
        ("base.json", include_str!("fixtures/model_layers/base.json")),
        ("user.json", include_str!("fixtures/model_layers/user.json")),
    ] {
        let path = scratch.path().join(name);
        std::fs::write(&path, text).unwrap();
        paths.push(path.to_string_lossy().into_owned());
    }
    let registry = ModelRegistry::with_model_files(
        support::auth(),
        paths,
        NativeModelFileOperations,
        support::values(),
    )
    .unwrap();
    assert_layers(&registry);
    registry.refresh().unwrap();
    assert_layers(&registry);
    registry
        .register_provider(
            "layered",
            Arc::new(RwLock::new(ProviderConfigInput {
                base_url: Some("https://dynamic.invalid".into()),
                ..Default::default()
            })),
        )
        .unwrap();
    assert_eq!(
        registry
            .find("layered", "retained")
            .unwrap()
            .read()
            .unwrap()
            .base_url,
        "https://dynamic.invalid"
    );
    registry.unregister_provider("layered").unwrap();
    assert_layers(&registry);
}
