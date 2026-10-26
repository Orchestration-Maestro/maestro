//! Provider operations exercised through the production component loader.
#![cfg(test)]
use maestro_extensions::types::ExtensionHandle;
use maestro_extensions::{ExtensionRunner, Local};
use serde_json::{Value, json};

/// Calls the retained author command and decodes its observation.
async fn observe(runner: &ExtensionRunner<Local>, request: &str) -> Value {
    let handler = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let result = handler(request.into(), runner.create_command_context()).await;
    let text = result.unwrap_err().to_string();
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{error}: {text}"))
}

/// Keeps the loaded owner alive while the original catalog is observed.
async fn loaded() -> (ExtensionHandle<Local>, ExtensionRunner<Local>) {
    let bus = super::components::TracedBus::new();
    let (owner, runner, _) =
        super::retained_components::loaded_on(bus.controller.for_extension()).await;
    (owner, runner)
}

#[test]
fn provider_author_component_uses_native_catalog_without_adapter_changes() {
    super::components::local(async {
        let (_owner, runner) = loaded().await;
        let answer = observe(&runner, "provider catalog").await;
        assert_eq!(
            answer,
            json!({
                "label":" Literal provider ", "key":"literal-key",
                "configured":true, "status":{"configured":true,"source":"models_json_key"},
                "oauth":false, "before":["first"], "replaced":["second"],
                "refreshed":["second"], "removed":true, "error":null,
                "auth":{"ok":true,"apiKey":"literal-key","headers":{"x-test":"literal","Authorization":"Bearer literal-key"}}
            })
        );
    });
}

#[test]
fn credential_author_retains_callable_native_owner() {
    super::components::local(async {
        let bus = super::components::TracedBus::new();
        let (session, _) = super::session::runner_session_with_config(
            "/credential-owner",
            maestro_extensions::SharedHandle::new(CredentialEffects),
        );
        let (_owner, runner, _) =
            super::retained_components::loaded_in(bus.controller.for_extension(), session).await;
        assert_eq!(
            observe(&runner, "credential owner").await,
            json!({"key":"original effect key","stored":{"type":"api_key","key":"!checkpoint-credential"},
                "listed":["author-credential"],"has":true,"auth":true,
                "status":{"configured":true,"source":"stored"},"errors":[]})
        );
        assert_eq!(
            observe(&runner, "credential retained").await,
            json!("original effect key")
        );
    });
}

/// A supplied credential effect that cannot be reproduced by an ambient shell lookup.
struct CredentialEffects;
impl maestro_extensions::ConfigValueOperations for CredentialEffects {
    fn execute(&self, command: &str) -> Option<Vec<u8>> {
        assert_eq!(command, "checkpoint-credential");
        Some(b" original effect key ".to_vec())
    }
}
