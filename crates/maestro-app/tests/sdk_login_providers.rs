//! The login provider registry a created session and an earlier catalog find.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::no_commands::no_commands;
use crate::support::sdk_sample::{Error, execute};
use maestro_app::{AuthCredential, AuthStorage, ModelRegistry};
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::Arc;

/// A catalog built over a stored login, before any session exists, applies that login's
/// model changes.
#[test]
fn catalogs_built_before_a_session_apply_the_stored_login() {
    let auth = Arc::new(AuthStorage::in_memory(std::iter::empty().collect()));
    auth.set(
        "github-copilot",
        AuthCredential::OAuth(maestro_models::OAuthCredentials {
            access: "proxy-ep=proxy.example.invalid;".into(),
            refresh: "unused-refresh".into(),
            expires: 0.0,
            extra: maestro_models::JsonObject::new(),
        }),
    );

    let registry = ModelRegistry::in_memory(auth, no_commands()).unwrap();

    let models = registry.get_all();
    let base_urls: BTreeSet<String> = models
        .read()
        .unwrap()
        .iter()
        .map(|model| model.read().unwrap().clone())
        .filter(|model| model.provider == "github-copilot")
        .map(|model| model.base_url)
        .collect();
    assert_eq!(
        base_urls,
        BTreeSet::from(["https://api.example.invalid".to_owned()])
    );
}

/// Session creation alone leaves the built-in login providers in their positions.
#[test]
fn created_sessions_find_the_built_in_login_providers_in_their_positions() -> Result<(), Error> {
    let identifiers = execute(|sample| async move {
        let created = maestro_app::create_agent_session(sample.options()?).await?;
        created.session.dispose();
        let identifiers: Vec<String> = maestro_models::get_oauth_providers()
            .iter()
            .map(|provider| provider.id().to_owned())
            .collect();
        Ok(json!(identifiers))
    })?;

    assert_eq!(
        identifiers,
        json!(["anthropic", "github-copilot", "openai-codex"])
    );
    Ok(())
}
