//! The `/login` method and provider choices and API-key entry.

use super::super::models::{editor_focused, refuse_refresh, register, register_configured};
use super::super::typed::{poll, recalled, waiting};
use super::{
    WIDE, answer, choose_method, choose_provider, chooser, dialog, document_store, editor_restored,
    notes, panel_rows, picker, quiet_browser, saved_path, settle, sign_in_to, storage, stored_key,
    subscribe,
};
use super::{World, isolated, isolated_case, started_with};
use crate::AuthType;
use maestro_app::{AuthCredential, OAuthCredentials, OAuthError};
use maestro_theme::ThemeColor;
use maestro_tui::Component;
use std::{path::Path, sync::Arc};

/// Registers the catalog provider `id` with two models, named `name` when given and keyed
/// by `key`, or by a configured literal key when absent.
fn keyed(world: &World, id: &str, name: Option<&str>, key: Option<&str>) {
    let (name, key) = (
        name.map(str::to_owned),
        key.unwrap_or("configured").to_owned(),
    );
    register_configured(world, id, &["one", "two"], move |config| {
        config.name = name;
        config.api_key = Some(key);
    });
}

/// A subscription that never finishes signing in.
fn waiting_subscription(world: &World, id: &str, name: &str) {
    subscribe(world, (id, name), None, |_| {
        Box::pin(std::future::pending::<Result<OAuthCredentials, OAuthError>>())
    });
}

/// The offered `(id, name, kind)` rows for `kind` among `ids`, in offered order.
fn offered(world: &World, kind: Option<AuthType>, ids: &[&str]) -> Vec<(String, String, String)> {
    world
        .mode
        .get_login_provider_options(kind)
        .unwrap()
        .into_iter()
        .filter(|option| ids.contains(&option.id.as_str()))
        .map(|option| {
            let shown = super::kind(&option).to_owned();
            (option.id, option.name, shown)
        })
        .collect()
}

/// Expected `(id, name, kind)` rows.
fn rows(expected: &[(&str, &str, &str)]) -> Vec<(String, String, String)> {
    expected
        .iter()
        .map(|(id, name, kind)| ((*id).to_owned(), (*name).to_owned(), (*kind).to_owned()))
        .collect()
}

/// The providers the row witness registers, and the built-ins it also reads.
const ROW_IDS: [&str; 15] = [
    "custom-oauth",
    "second-key",
    "first-key",
    "custom-key",
    "a",
    "b",
    "c",
    "d",
    "e",
    "f",
    "g",
    "h",
    "anthropic",
    "github-copilot",
    "openai-codex",
];

/// Registers a subscription and key providers with equal, empty, accented, decomposed,
/// emoji and case-differing names, each keyed by a command that must not run.
fn named_catalog(world: &World) {
    waiting_subscription(world, "custom-oauth", "Same");
    for (id, name) in [
        ("second-key", Some("Same")),
        ("first-key", Some("Same")),
        ("custom-key", None),
        ("a", Some("zebra")),
        ("b", Some("Ångström")),
        ("c", Some("alpha")),
        ("d", Some("Alpha")),
        ("e", Some("éclair")),
        ("f", Some("e\u{301}clair")),
        ("g", Some("😀")),
        ("h", Some("")),
    ] {
        keyed(world, id, name, Some("!touch executed"));
    }
}

/// Checks the key rows: locale name order, stable ties and the built-in exclusions.
fn key_rows(world: &World) -> Vec<(String, String, String)> {
    let expected = rows(&[
        ("h", "", "api_key"),
        ("g", "😀", "api_key"),
        ("c", "alpha", "api_key"),
        ("d", "Alpha", "api_key"),
        ("b", "Ångström", "api_key"),
        ("custom-key", "custom-key", "api_key"),
        ("e", "éclair", "api_key"),
        ("f", "e\u{301}clair", "api_key"),
        ("second-key", "Same", "api_key"),
        ("first-key", "Same", "api_key"),
        ("a", "zebra", "api_key"),
    ]);
    let offered_keys = offered(world, Some(AuthType::ApiKey), &ROW_IDS);
    let mut custom = offered_keys.clone();
    custom.retain(|row| row.0 != "anthropic");
    assert_eq!(custom, expected, "key rows keep name order and stable ties");
    assert!(
        offered_keys.iter().any(|row| row.0 == "anthropic"),
        "a labelled built-in keeps key sign-in"
    );
    let excluded = ["github-copilot", "openai-codex"];
    assert!(
        !offered_keys
            .iter()
            .any(|row| excluded.contains(&row.0.as_str())),
        "subscription-only built-ins offer no key"
    );
    offered_keys
}

#[test]
fn login_provider_rows_keep_method_identity_and_name_order() {
    // The rows follow English collation, as the logout listing does; the pin states it.
    isolated_case(
        "login_provider_rows_keep_method_identity_and_name_order",
        "child",
        &[("LC_ALL", "en_US.UTF-8")],
        async |directory| {
            let world = started_with(directory, None).await;
            named_catalog(&world);
            let keys = key_rows(&world);
            let subscriptions = offered(&world, Some(AuthType::OAuth), &ROW_IDS);
            for id in [
                "custom-oauth",
                "anthropic",
                "github-copilot",
                "openai-codex",
            ] {
                assert!(
                    subscriptions
                        .iter()
                        .any(|row| row.0 == id && row.2 == "oauth")
                );
            }
            let all = offered(&world, None, &ROW_IDS);
            assert_eq!(all.len(), subscriptions.len() + keys.len());
            let same: Vec<_> = all
                .iter()
                .filter(|row| row.1 == "Same")
                .map(|row| row.0.as_str())
                .collect();
            let order = ["custom-oauth", "second-key", "first-key"];
            assert_eq!(same, order, "subscriptions precede keys among equal names");
            let twice = all.iter().filter(|row| row.0 == "anthropic").count();
            assert_eq!(twice, 2, "one provider may appear once per method");
            assert!(
                !directory.join("executed").exists(),
                "listing runs no key command"
            );
        },
    );
}

#[test]
fn login_command_opens_method_choices_without_prompt_delivery() {
    isolated(
        "login_command_opens_method_choices_without_prompt_delivery",
        async |directory| {
            for text in [
                "/login",
                "\u{feff}/login",
                "\u{a0}/login\u{a0}",
                " /login\t",
            ] {
                let world = started_with(directory, None).await;
                let mut waiter = waiting(&world.mode);
                world.mode.editor.set_text(text);
                super::submit(&world.mode, text);
                assert_eq!(poll(&mut waiter), None, "{text:?} answers no prompt");
                assert!(
                    recalled(&world.mode.editor).is_empty(),
                    "{text:?} stays out"
                );
                let shown = chooser(&world).expect("the method choice is shown");
                let rows = shown.render(WIDE).concat();
                assert!(rows.contains("Select authentication method:"));
                assert!(rows.contains("Use a subscription") && rows.contains("Use an API key"));
                assert_eq!(world.mode.editor.get_text(), "");
                world.terminal.clear_writes();
                world.terminal.send_input("\x1b");
                world.runtime.settle().unwrap();
                assert!(editor_restored(&world), "cancel puts the editor back");
                assert!(!world.terminal.writes().is_empty(), "a frame is requested");
                assert!(editor_focused(&world));
            }
            for text in ["/login extra", "/logins", "/LOGIN", "\u{85}/login\u{85}"] {
                let world = started_with(directory, None).await;
                let mut waiter = waiting(&world.mode);
                world.mode.editor.set_text(text);
                super::submit(&world.mode, text);
                assert_eq!(
                    poll(&mut waiter),
                    Some(text.to_owned()),
                    "{text:?} is input"
                );
                assert!(chooser(&world).is_none());
            }
        },
    );
}

#[test]
fn login_provider_navigation_routes_all_authentication_kinds() {
    isolated(
        "login_provider_navigation_routes_all_authentication_kinds",
        async |directory| {
            quiet_browser(directory);
            let world = started_with(directory, None).await;
            waiting_subscription(&world, "acme", "Acme");
            keyed(&world, "custom-key", Some("Custom"), None);
            choose_method(&world, AuthType::OAuth);
            assert!(picker(&world).is_some());
            world.terminal.send_input("\x1b");
            assert!(
                chooser(&world).is_some(),
                "cancel returns to the method choice"
            );
            world.terminal.send_input("\r");
            choose_provider(&world, AuthType::OAuth, "acme");
            assert!(
                panel_rows(&world).contains("Login to Acme"),
                "subscription dialog"
            );
            assert!(!editor_focused(&world), "the dialog keeps focus");
            for (id, shown) in [
                ("custom-key", "Enter API key:"),
                ("amazon-bedrock", "Amazon Bedrock setup"),
            ] {
                let world = started_with(directory, None).await;
                keyed(&world, "custom-key", Some("Custom"), None);
                sign_in_to(&world, AuthType::ApiKey, id);
                let dialog = dialog(&world);
                assert!(dialog.render(WIDE).concat().contains(shown), "{id}");
                assert!(storage(&world).get(id).is_none(), "{id} stores nothing yet");
            }
        },
    );
}

#[test]
fn login_empty_method_lists_report_distinct_statuses() {
    isolated(
        "login_empty_method_lists_report_distinct_statuses",
        async |directory| {
            let world = started_with(directory, None).await;
            let session = world.host.session();
            session.model_registry().get_all().write().unwrap().clear();
            choose_method(&world, AuthType::ApiKey);
            assert!(notes(&world, "No API key providers available."));
            assert!(!notes(&world, "No subscription providers available."));
            assert!(picker(&world).is_none(), "no empty picker opens");
            assert!(editor_restored(&world));
            assert!(storage(&world).list().is_empty());
        },
    );
}

#[test]
fn bedrock_setup_keeps_guidance_and_restores_editor_on_close() {
    isolated(
        "bedrock_setup_keeps_guidance_and_restores_editor_on_close",
        async |directory| {
            let world = started_with(directory, None).await;
            let prior = super::super::models::current(&world);
            sign_in_to(&world, AuthType::ApiKey, "amazon-bedrock");
            let theme = world.themes.theme().get().unwrap();
            let docs = maestro_app::configuration::config::get_docs_path().unwrap();
            let guide = Path::new(&docs).join("providers.md");
            let rows = panel_rows(&world);
            for (color, text) in [
                (
                    ThemeColor::Text,
                    "Amazon Bedrock uses AWS credentials instead of a single API key.",
                ),
                (
                    ThemeColor::Text,
                    "Configure an AWS profile, IAM keys, bearer token, or role-based credentials.",
                ),
                (ThemeColor::Muted, "See:"),
                (ThemeColor::Accent, &format!("  {}", guide.display())),
            ] {
                assert!(rows.contains(&theme.fg(&color, text).unwrap()), "{text}");
            }
            assert!(rows.contains("Amazon Bedrock setup"));
            world.terminal.clear_writes();
            world.terminal.send_input("\x1b");
            world.runtime.settle().unwrap();
            assert!(editor_restored(&world), "closing puts the editor back");
            assert!(!world.terminal.writes().is_empty(), "a frame is requested");
            assert!(editor_focused(&world));
            assert!(storage(&world).list().is_empty(), "no key is stored");
            assert_eq!(
                super::super::models::current(&world),
                prior,
                "no model is selected"
            );
        },
    );
}

/// Every pasted key, with the stored key or `None` when it is refused as empty.
const PASTED_KEYS: &[(&str, Option<&str>)] = &[
    ("", None),
    (" \t\r\n", None),
    ("\u{feff}", None),
    (" \t fixture \r\n", Some("fixture")),
    ("\u{feff}fixture\u{feff}", Some("fixture")),
    ("\u{85}fixture\u{85}", Some("\u{85}fixture\u{85}")),
    ("\u{85}", Some("\u{85}")),
    ("\u{a0}fixture\u{a0}", Some("fixture")),
    ("alpha beta", Some("alpha beta")),
    ("\u{200b}", Some("\u{200b}")),
    (
        "\u{feff}\u{a0} \tfixture \r\n\u{a0}\u{feff}",
        Some("fixture"),
    ),
    (" é 中文 🧭 ", Some("é 中文 🧭")),
];

#[test]
fn api_key_prompt_uses_ecmascript_whitespace_boundaries() {
    isolated(
        "api_key_prompt_uses_ecmascript_whitespace_boundaries",
        async |directory| {
            for (case, (pasted, stored)) in PASTED_KEYS.iter().enumerate() {
                let world = started_with(directory, None).await;
                keyed(&world, "custom-key", None, None);
                sign_in_to(&world, AuthType::ApiKey, "custom-key");
                assert!(panel_rows(&world).contains("Enter API key:"));
                answer(&world, pasted);
                assert!(editor_restored(&world), "case {case}");
                assert_eq!(
                    stored_key(&world, "custom-key").as_deref(),
                    *stored,
                    "case {case}"
                );
                let refused =
                    "Error: Failed to save API key for custom-key: API key cannot be empty.";
                assert_eq!(notes(&world, refused), stored.is_none(), "case {case}");
            }
        },
    );
}

#[test]
fn api_key_save_restores_editor_before_catalog_completion() {
    isolated(
        "api_key_save_restores_editor_before_catalog_completion",
        async |directory| {
            let world = started_with(directory, None).await;
            keyed(&world, "custom-key", Some("Custom"), None);
            storage(&world).set(
                "other",
                AuthCredential::ApiKey(maestro_app::ApiKeyCredential {
                    env: None,
                    key: "kept".to_owned(),
                }),
            );
            sign_in_to(&world, AuthType::ApiKey, "custom-key");
            answer(&world, "secret");
            assert_eq!(stored_key(&world, "custom-key").as_deref(), Some("secret"));
            assert_eq!(stored_key(&world, "other").as_deref(), Some("kept"));
            assert!(editor_restored(&world));
            assert!(notes(
                &world,
                &format!(
                    "Saved API key for Custom. Credentials saved to {}",
                    saved_path()
                )
            ));
            assert!(notes(
                &world,
                "Error: Saved API key for Custom, but no default model is configured for provider \"custom-key\". Use /model to select a model."
            ));

            let world = started_with(directory, None).await;
            keyed(&world, "custom-key", Some("Custom"), None);
            sign_in_to(&world, AuthType::ApiKey, "custom-key");
            refuse_refresh(&world);
            answer(&world, "secret");
            assert_eq!(stored_key(&world, "custom-key").as_deref(), Some("secret"));
            assert!(
                editor_restored(&world),
                "the editor is back before completion fails"
            );
            assert!(notes(
                &world,
                "Error: Failed to save API key for Custom: refresh refused"
            ));
            assert!(!notes(&world, "Saved API key for Custom."));

            let world = started_with(directory, None).await;
            keyed(&world, "custom-key", Some("Custom"), None);
            sign_in_to(&world, AuthType::ApiKey, "custom-key");
            world.terminal.send_input("\x1b");
            settle(&world);
            assert!(editor_restored(&world), "cancelling puts the editor back");
            assert!(stored_key(&world, "custom-key").is_none());
            assert!(!notes(&world, "Error: "), "cancelling is silent");
        },
    );
}

#[test]
fn api_key_store_failure_remains_a_store_diagnostic() {
    isolated(
        "api_key_store_failure_remains_a_store_diagnostic",
        async |directory| {
            let auth = document_store(directory, 0, &[]);
            let world = started_with(directory, Some(Arc::clone(&auth))).await;
            keyed(&world, "custom-key", Some("Custom"), None);
            let file = directory.join("store0").join("auth.json");
            let before = std::fs::read_to_string(&file).unwrap();
            let Some(_restored) = maestro_test_directory::deny(file.clone(), 0o400).unwrap() else {
                return;
            };
            sign_in_to(&world, AuthType::ApiKey, "custom-key");
            answer(&world, "secret");
            assert_eq!(
                stored_key(&world, "custom-key").as_deref(),
                Some("secret"),
                "memory accepts it"
            );
            assert_eq!(
                std::fs::read_to_string(&file).unwrap(),
                before,
                "the file is unchanged"
            );
            assert_eq!(
                auth.drain_errors().len(),
                1,
                "one write failure stays drainable"
            );
            assert!(notes(
                &world,
                "Saved API key for Custom. Credentials saved to"
            ));
            assert!(!notes(&world, "Failed to save API key"));
        },
    );
}

#[test]
fn authentication_lists_use_bounded_linear_traversals() {
    isolated(
        "authentication_lists_use_bounded_linear_traversals",
        async |directory| {
            for scale in [16, 32, 64] {
                let world = started_with(directory, None).await;
                let ids: Vec<String> = (0..scale).map(|index| format!("model{index}")).collect();
                let borrowed: Vec<&str> = ids.iter().map(String::as_str).collect();
                for provider in ["p1", "p2"] {
                    register(&world, provider, &borrowed);
                }
                let offered = offered(&world, Some(AuthType::ApiKey), &["p1", "p2"]);
                assert_eq!(
                    offered.len(),
                    2,
                    "one row per provider at {scale} models each"
                );
            }
        },
    );
}
