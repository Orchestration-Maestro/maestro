//! The login provider registry a created session finds.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::sdk_sample::{Error, execute};
use serde_json::json;

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
