//! The login provider registry keeps the built-in providers in their positions through startup.
#![cfg(unix)]
use crate::support::entry::{Entry, Provider, cases};
use login_registry::listed;
use maestro_cli::startup::entrypoint::MainOptions;
use maestro_models::{register_oauth_provider, reset_oauth_providers, unregister_oauth_provider};
use named_account::Named;
use std::rc::Rc;
use std::sync::Arc;
use tokio::task::LocalSet;

#[path = "../../maestro-models/tests/support/login_registry.rs"]
mod login_registry;
#[path = "../../maestro-models/tests/support/named_account.rs"]
mod named_account;

/// The built-in providers as the registry lists them, in their positions.
const BUILT_INS: [&str; 3] = [
    "anthropic: Anthropic (Claude Pro/Max)",
    "github-copilot: GitHub Copilot",
    "openai-codex: ChatGPT Plus/Pro (Codex Subscription)",
];

/// The API protocols in their built-in registration positions.
const APIS: [&str; 9] = [
    "anthropic-messages",
    "openai-completions",
    "mistral-conversations",
    "openai-responses",
    "azure-openai-responses",
    "openai-codex-responses",
    "google-generative-ai",
    "google-vertex",
    "bedrock-converse-stream",
];

/// Snapshot API membership in dispatch order.
fn api_ids() -> Vec<String> {
    maestro_models::get_api_providers()
        .into_iter()
        .map(|provider| provider.api)
        .collect()
}

/// Start `mode` through the public entry and wait for its presentation to end.
async fn compose(e: &Entry, local: &Rc<LocalSet>, mode: &str) {
    let status = match mode {
        "print" => {
            let provider = Provider::start(vec![200]);
            e.served_models(&provider.base_url, &["first"]);
            *e.tty.borrow_mut() = [false].into();
            *e.stdin.borrow_mut() = b"prompt".to_vec();
            let args = ["-p", "--model", "startup/first"];
            let status = e.run(local, &args, MainOptions::default()).await;
            assert_eq!(provider.join().len(), 1);
            status
        }
        "rpc" => {
            e.models(&["first"]);
            *e.tty.borrow_mut() = [false].into();
            *e.stdin.borrow_mut() = b"{\"type\":\"get_state\"}\n".to_vec();
            e.run(local, &["--mode", "rpc"], MainOptions::default())
                .await
        }
        _ => {
            e.models(&["first"]);
            e.run(local, &[], MainOptions::default()).await
        }
    };
    assert_eq!(status.unwrap(), 0, "{mode}");
    let launched = if mode == "rpc" { "rpc" } else { "factory" };
    assert_eq!(
        e.log().iter().any(|entry| entry == launched),
        mode != "print",
        "{mode}"
    );
}

#[test]
fn built_in_login_providers_hold_their_positions_after_each_mode_starts() {
    let modes: &[(&str, &[(&str, &str)])] = &[("interactive", &[]), ("print", &[]), ("rpc", &[])];
    cases(
        "login_provider_order::built_in_login_providers_hold_their_positions_after_each_mode_starts",
        modes,
        |local, mode| async move {
            compose(&Entry::new(), &local, mode).await;

            assert_eq!(listed(), BUILT_INS, "{mode}");
            assert_eq!(api_ids(), APIS, "{mode}");
        },
    );
}

#[test]
fn registrations_made_before_startup_keep_the_built_in_positions() {
    let modes: &[(&str, &[(&str, &str)])] = &[("interactive", &[]), ("print", &[]), ("rpc", &[])];
    cases(
        "login_provider_order::registrations_made_before_startup_keep_the_built_in_positions",
        modes,
        |local, mode| async move {
            register_oauth_provider(Arc::new(Named("custom-login", "Custom")));
            register_oauth_provider(Arc::new(Named("github-copilot", "Replacement")));
            let custom = maestro_models::register_faux_provider(
                maestro_models::RegisterFauxProviderOptions {
                    api: Some("custom-api".into()),
                    ..Default::default()
                },
            );
            let replacement = maestro_models::register_faux_provider(
                maestro_models::RegisterFauxProviderOptions {
                    api: Some("anthropic-messages".into()),
                    ..Default::default()
                },
            );
            let selected = maestro_models::get_api_provider("anthropic-messages").unwrap();

            compose(&Entry::new(), &local, mode).await;

            let shadowed = [
                BUILT_INS[0],
                "github-copilot: Replacement",
                BUILT_INS[2],
                "custom-login: Custom",
            ];
            assert_eq!(listed(), shadowed, "{mode}");
            let mut expected = APIS.map(str::to_owned).to_vec();
            expected.push("custom-api".into());
            assert_eq!(api_ids(), expected, "{mode}");
            assert!(Arc::ptr_eq(
                &selected.stream,
                &maestro_models::get_api_provider("anthropic-messages")
                    .unwrap()
                    .stream
            ));
            replacement.unregister();
            custom.unregister();
            assert_eq!(api_ids(), APIS, "{mode}: removed");
            assert!(!Arc::ptr_eq(
                &selected.stream,
                &maestro_models::get_api_provider("anthropic-messages")
                    .unwrap()
                    .stream
            ));
            unregister_oauth_provider("github-copilot");
            unregister_oauth_provider("custom-login");
            assert_eq!(listed(), BUILT_INS, "{mode}: removed");
        },
    );
}

#[test]
fn resetting_after_startup_leaves_only_the_built_in_login_providers() {
    let modes: &[(&str, &[(&str, &str)])] = &[("interactive", &[])];
    cases(
        "login_provider_order::resetting_after_startup_leaves_only_the_built_in_login_providers",
        modes,
        |local, mode| async move {
            register_oauth_provider(Arc::new(Named("anthropic", "Replacement")));
            compose(&Entry::new(), &local, mode).await;
            register_oauth_provider(Arc::new(Named("custom-login", "Custom")));

            let custom = maestro_models::register_faux_provider(
                maestro_models::RegisterFauxProviderOptions {
                    api: Some("custom-reset".into()),
                    ..Default::default()
                },
            );
            reset_oauth_providers();
            maestro_providers::reset_api_providers();

            assert_eq!(listed(), BUILT_INS);
            assert_eq!(api_ids(), APIS);
            custom.unregister();
        },
    );
}
