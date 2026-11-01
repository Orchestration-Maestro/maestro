//! Install telemetry preference and bounded native reporting.
use maestro_settings::SettingsManager;

/// Resolve an observed environment override against the existing preference.
#[must_use]
pub fn is_install_telemetry_enabled(
    settings_manager: &SettingsManager,
    telemetry_env: Option<&str>,
) -> bool {
    telemetry_env.map_or_else(
        || settings_manager.get_enable_install_telemetry(),
        |value| maestro_text::is_truthy_env_flag(Some(value)),
    )
}

#[cfg(not(target_arch = "wasm32"))]
use {
    super::{config::service_endpoint, maestro_user_agent::get_maestro_user_agent},
    crate::utils::uri::encode_uri_component,
    maestro_models::{BoxFuture, Fetch, HttpRequest, default_fetch, providers::http::race},
    std::time::Duration,
};

/// Capture current reporting policy and return owned request work without spawning it.
/// The request ends at response headers; its outcome is discarded. `offline` is the
/// caller's startup selection, or `MAESTRO_OFFLINE` when absent.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn report_install_telemetry(
    version: &str,
    settings_manager: &SettingsManager,
    service_origin: &str,
    fetch: Option<Fetch>,
    offline: Option<bool>,
) -> BoxFuture<()> {
    if offline
        .unwrap_or_else(|| super::flag_is_set(std::env::var("MAESTRO_OFFLINE").ok().as_deref()))
        || !is_install_telemetry_enabled(
            settings_manager,
            std::env::var("MAESTRO_TELEMETRY").ok().as_deref(),
        )
    {
        return Box::pin(async {});
    }
    let Some(endpoint) = service_endpoint(service_origin, "/api/report-install") else {
        return Box::pin(async {});
    };
    let request = HttpRequest {
        method: "GET".into(),
        url: format!("{endpoint}?version={}", encode_uri_component(version)),
        headers: [(
            "user-agent".into(),
            get_maestro_user_agent(version, std::env::consts::OS, std::env::consts::ARCH),
        )]
        .into(),
        body: Vec::new(),
        signal: None,
    };
    let fetch = fetch.unwrap_or_else(default_fetch);
    Box::pin(async move {
        let _ = race(fetch(request), Some(Duration::from_millis(5_000)), None).await;
    })
}
