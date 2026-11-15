//! Base-layer protocol order beneath live registrations.

use crate::registry::registered;
use maestro_models::{
    ApiProvider, RegisterFauxProviderOptions, SharedHandle, clear_api_providers, get_api_provider,
    get_api_providers, register_faux_provider,
};

/// Retain callbacks from the scripted provider without retaining its registration.
fn provider(api: &str) -> ApiProvider {
    let registration = register_faux_provider(RegisterFauxProviderOptions {
        api: Some(api.into()),
        ..Default::default()
    });
    let provider = get_api_provider(api).unwrap();
    registration.unregister();
    provider
}

#[test]
fn installed_api_base_keeps_positions_under_existing_registrations() {
    let _serial = crate::registry::registry();
    clear_api_providers();
    let base = [provider("base-first"), provider("base-second")];
    let extra = register_faux_provider(RegisterFauxProviderOptions {
        api: Some("custom-last".into()),
        ..Default::default()
    });
    let replacement = register_faux_provider(RegisterFauxProviderOptions {
        api: Some("base-second".into()),
        ..Default::default()
    });
    let selected = get_api_provider("base-second").unwrap();

    maestro_models::install_built_in_api_providers(base.clone());
    assert_eq!(registered(), ["base-first", "base-second", "custom-last"]);
    assert!(SharedHandle::ptr_eq(
        &selected.stream,
        &get_api_provider("base-second").unwrap().stream
    ));
    maestro_models::install_built_in_api_providers(base);
    assert_eq!(registered(), ["base-first", "base-second", "custom-last"]);
    replacement.unregister();
    assert_eq!(registered(), ["base-first", "base-second", "custom-last"]);
    assert!(!SharedHandle::ptr_eq(
        &selected.stream,
        &get_api_provider("base-second").unwrap().stream
    ));
    extra.unregister();
    assert_eq!(registered(), ["base-first", "base-second"]);
    clear_api_providers();
    assert!(get_api_providers().is_empty());
}
