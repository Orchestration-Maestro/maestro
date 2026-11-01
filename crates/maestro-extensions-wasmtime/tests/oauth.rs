//! OAuth capabilities exercised through a compiled guest and the production host.
#![cfg(test)]
use serde_json::{Value, json};

#[test]
fn oauth_provider_enumeration_keeps_metadata_and_callable_keys() {
    super::components::local(async {
        let bus = super::components::TracedBus::new();
        let (_owner, runner, _) =
            super::retained_components::loaded_on(bus.controller.for_extension()).await;
        let command = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        let answer = command(
            "credential oauth providers".into(),
            runner.create_command_context(),
        )
        .await
        .unwrap_err()
        .to_string();
        let answer: Value = serde_json::from_str(&answer).unwrap();
        assert_eq!(
            answer,
            json!([
                {"id":"anthropic", "name":"Anthropic (Claude Pro/Max)", "usesCallbackServer":true, "key":"literal-access"},
                {"id":"github-copilot", "name":"GitHub Copilot", "usesCallbackServer":null, "key":"literal-access"},
                {"id":"openai-codex", "name":"ChatGPT Plus/Pro (Codex Subscription)", "usesCallbackServer":true, "key":"literal-access"}
            ])
        );
    });
}

mod refresh;

#[test]
fn oauth_refresh_and_model_modification_use_the_native_owner() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let server = refresh::Server::new();
        maestro_models::register_oauth_provider(std::sync::Arc::new(refresh::Provider(server.url.clone())));
        let bus = super::components::TracedBus::new();
        let (_owner, runner, _) = super::retained_components::loaded_on(bus.controller.for_extension()).await;
        let command = runner.get_command("bridge-observe").unwrap().command.handler;
        let answer = command("credential oauth operations".into(), runner.create_command_context()).await.unwrap_err().to_string();
        let answer: Value = serde_json::from_str(&answer).unwrap_or_else(|_| panic!("{answer}"));
        assert_eq!(answer, json!({
            "credentials":{"refresh":"literal-refresh", "access":"proxy-ep=proxy.controlled.example;native-token", "expires":4_102_444_500_000_f64,
                "enterpriseUrl":"company.ghe.com","extensionData":{"nested":[null,false,-0.0]}},
            "key":"proxy-ep=proxy.controlled.example;native-token", "baseUrls":["https://api.controlled.example","https://before.invalid"],
            "neighbor":true,"absent":true
        }));
        server.finish();
        maestro_models::unregister_oauth_provider("component-refresh");
    });
}

mod lifetime;

#[test]
fn oauth_provider_outlives_membership_and_releases_once() {
    super::components::local(async {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let drops = Arc::new(AtomicUsize::new(0));
        maestro_models::register_oauth_provider(Arc::new(lifetime::Provider(drops.clone())));
        let bus = super::components::TracedBus::new();
        let (_owner, runner, _) =
            super::retained_components::loaded_on(bus.controller.for_extension()).await;
        assert_eq!(
            call(&runner, "retain").await,
            json!(["component-retained", " Literal login label ", false])
        );
        maestro_models::unregister_oauth_provider("component-retained");
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        runner.invalidate(Some("original context retired".into()));
        assert_eq!(
            call(&runner, "key").await,
            json!(["literal-access", "literal key denial"])
        );
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(call(&runner, "release").await, json!(true));
        let command = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        command("barrier".into(), runner.create_command_context())
            .await
            .unwrap_err();
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(call(&runner, "release").await, json!(false));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    });
}

/// Observes a retained provider without acquiring a new context-owned catalog.
async fn call(
    runner: &maestro_extensions::ExtensionRunner<maestro_extensions::Local>,
    operation: &str,
) -> Value {
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let answer = command(
        format!("credential oauth {operation}"),
        runner.create_command_context(),
    )
    .await
    .unwrap_err()
    .to_string();
    serde_json::from_str(&answer).unwrap_or_else(|_| panic!("{answer}"))
}

#[test]
fn oauth_refresh_outlives_its_initiating_guest_handle() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let server = refresh::Server::new();
        maestro_models::register_oauth_provider(std::sync::Arc::new(refresh::Provider(
            server.url.clone(),
        )));
        let bus = super::components::TracedBus::new();
        let (_owner, runner, _) =
            super::retained_components::loaded_on(bus.controller.for_extension()).await;
        assert_eq!(call(&runner, "pending create").await, json!("retained"));
        maestro_models::unregister_oauth_provider("component-refresh");
        runner.invalidate(Some("initiating context retired".into()));
        assert_eq!(
            call(&runner, "pending complete").await,
            json!(["proxy-ep=proxy.controlled.example;native-token", true])
        );
        server.finish();
    });
}

mod login;
mod oauth_callback;
mod reverse;

#[test]
fn oauth_guest_registration_modifies_models_and_registers_facade() {
    super::components::local(async {
        let bus = super::components::TracedBus::new();
        let (_owner, runner, _) =
            super::retained_components::loaded_on(bus.controller.for_extension()).await;
        let result = call(&runner, "registration").await;
        assert_eq!(result["models"], json!(["https://guest.invalid/modified"]));
        assert_eq!(result["facade"], "guest-computed:facade:r");
    });
}

#[test]
fn oauth_guest_registration_computes_owned_keys_and_refreshes() {
    super::components::local(async {
        let bus = super::components::TracedBus::new();
        let (_owner, runner, _) =
            super::retained_components::loaded_on(bus.controller.for_extension()).await;
        assert_eq!(
            call(&runner, "registration").await,
            json!({
                "models":["https://guest.invalid/modified"], "facade":"guest-computed:facade:r",
                "id":"guest-computed", "name":" Guest account ", "server":false,
                "key":"guest-computed:a:r", "rotatedKey":"guest-computed:a-rotated:r",
                "loggedIn":{"type":"oauth","refresh":"r","access":"code","expires":0.0,
                    "open":{"nested":[null,false,-0.0]}},
                "credentials":{"refresh":"r","access":"a-rotated","expires":4_102_444_500_000_f64,
                    "open":{"nested":[null,false,-0.0]}}
            })
        );
    });
}
