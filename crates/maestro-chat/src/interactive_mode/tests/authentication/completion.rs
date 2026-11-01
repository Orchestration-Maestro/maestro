//! What a successful sign-in changes: the default model, the provider count and notices.

use super::super::models::{
    current, refuse_availability, refuse_recording, refuse_refresh, register, scope, sign_in,
};
use super::{
    World, count, isolated, notes, refuse_after, saved_path, settle, sign_in_to, signed,
    started_with, subscribe,
};
use crate::AuthType;
use maestro_app::{DEFAULT_MODEL_PER_PROVIDER, Model, OAuthCredentials, OAuthError};
use std::{
    error::Error,
    sync::{Arc, Mutex, RwLock},
};
use tokio::sync::oneshot;

/// The subscription billing notice.
const WARNING: &str = "Anthropic subscription auth is active.";

/// Both completion labels, with the text each one starts with for the provider `Name`.
const LABELS: [(AuthType, &str); 2] = [
    (AuthType::OAuth, "Logged in to Name"),
    (AuthType::ApiKey, "Saved API key for Name"),
];

/// The configured default model of `provider`.
fn default_of(provider: &str) -> String {
    DEFAULT_MODEL_PER_PROVIDER.read().unwrap()[provider].clone()
}

/// A fresh descriptor equal to the placeholder a session starts with.
fn placeholder(world: &World) -> Arc<RwLock<Model>> {
    let model = world.host.session().model().read().unwrap().clone();
    assert_eq!(
        (
            model.provider.as_str(),
            model.id.as_str(),
            model.api.as_str()
        ),
        ("unknown", "unknown", "unknown"),
        "the session starts on the placeholder"
    );
    Arc::new(RwLock::new(model))
}

/// Completes signing in to `provider` as `Name` with `kind` after `prior`.
async fn complete(
    world: &World,
    provider: &str,
    kind: AuthType,
    prior: Arc<RwLock<Model>>,
) -> Result<(), Box<dyn Error>> {
    world
        .mode
        .complete_provider_authentication(provider, "Name", kind, prior)
        .await
}

/// The status a completion without a selection shows.
fn saved(label: &str) -> String {
    format!("{label}. Credentials saved to {}", saved_path())
}

/// The status a completion selecting `id` shows.
fn selected(label: &str, id: &str) -> String {
    format!(
        "{label}. Selected {id}. Credentials saved to {}",
        saved_path()
    )
}

/// A placeholder descriptor with `field` changed, or the exact placeholder for `None`.
fn changed(world: &World, field: Option<&str>) -> Arc<RwLock<Model>> {
    let prior = placeholder(world);
    let mut model = prior.write().unwrap();
    match field {
        Some("provider") => model.provider = "other".into(),
        Some("id") => model.id = "other".into(),
        Some("api") => model.api = "other".into(),
        _ => {}
    }
    drop(model);
    prior
}

/// Completes `kind` after a prior descriptor with `field` changed and checks whether the
/// provider's default was selected.
async fn sentinel_case(
    directory: &std::path::Path,
    (kind, label): (AuthType, &str),
    field: Option<&str>,
) {
    let default = default_of("openai");
    let world = started_with(directory, None).await;
    register(&world, "openai", &[&default]);
    let before = current(&world);
    let prior = changed(&world, field);
    complete(&world, "openai", kind, prior).await.unwrap();
    assert_eq!(
        count(&world).to_bits(),
        1f64.to_bits(),
        "{field:?}: counted"
    );
    if field.is_none() {
        assert_eq!(current(&world), format!("openai/{default}"));
        assert!(notes(&world, &selected(label, &default)));
        return;
    }
    assert_eq!(current(&world), before, "{field:?} keeps the model");
    assert!(notes(&world, &saved(label)), "{field:?}");
    assert!(!notes(&world, "Selected"), "{field:?}");
}

#[test]
fn authentication_completion_replaces_only_the_exact_unknown_sentinel() {
    isolated(
        "authentication_completion_replaces_only_the_exact_unknown_sentinel",
        async |directory| {
            for label in LABELS {
                for field in [None, Some("provider"), Some("id"), Some("api")] {
                    sentinel_case(directory, label, field).await;
                }
            }
        },
    );
}

#[test]
fn authentication_completion_reports_missing_provider_default() {
    isolated(
        "authentication_completion_reports_missing_provider_default",
        async |directory| {
            for (kind, label) in LABELS {
                for models in [&[][..], &["one"][..]] {
                    let world = started_with(directory, None).await;
                    register(&world, "custom", models);
                    complete(&world, "custom", kind, placeholder(&world))
                        .await
                        .unwrap();
                    assert!(notes(&world, &saved(label)));
                    let error = format!(
                        "Error: {label}, but no default model is configured for provider \"custom\". Use /model to select a model."
                    );
                    assert!(notes(&world, &error), "{models:?}");
                }
            }
        },
    );
}

#[test]
fn authentication_completion_reports_no_available_provider_models() {
    isolated(
        "authentication_completion_reports_no_available_provider_models",
        async |directory| {
            let default = default_of("openai");
            for (kind, label) in LABELS {
                let world = started_with(directory, None).await;
                register(&world, "decoy", &[&default]);
                let before = current(&world);
                complete(&world, "openai", kind, placeholder(&world))
                    .await
                    .unwrap();
                let error = format!(
                    "Error: {label}, but no models are available for that provider. Use /model to select a model."
                );
                assert!(notes(&world, &error));
                assert_eq!(
                    current(&world),
                    before,
                    "another provider's equal id is not chosen"
                );
            }
        },
    );
}

#[test]
fn authentication_completion_reports_unavailable_default_model() {
    isolated(
        "authentication_completion_reports_unavailable_default_model",
        async |directory| {
            let default = default_of("openai");
            for (kind, label) in LABELS {
                let world = started_with(directory, None).await;
                register(&world, "openai", &["not-the-default"]);
                let before = current(&world);
                complete(&world, "openai", kind, placeholder(&world))
                    .await
                    .unwrap();
                let error = format!(
                    "Error: {label}, but its default model \"{default}\" is not available. Use /model to select a model."
                );
                assert!(notes(&world, &error));
                assert_eq!(current(&world), before, "no other model is chosen");
            }
        },
    );
}

#[test]
fn authentication_completion_reports_default_selection_failure() {
    isolated(
        "authentication_completion_reports_default_selection_failure",
        async |directory| {
            let default = default_of("openai");
            for (kind, label) in LABELS {
                let world = started_with(directory, None).await;
                register(&world, "openai", &[&default]);
                refuse_recording(&world, directory);
                complete(&world, "openai", kind, placeholder(&world))
                    .await
                    .unwrap();
                let prefix = format!("Error: {label}, but selecting its default model failed: ");
                assert!(notes(&world, &prefix), "the setter's failure is reported");
                assert!(notes(&world, ". Use /model to select a model."));
                assert!(notes(&world, &saved(label)));
                assert!(!notes(&world, "Selected"), "no success is reported");
                assert_eq!(
                    current(&world),
                    format!("openai/{default}"),
                    "the setter's own change stays"
                );
            }
        },
    );
}

#[test]
fn authentication_completion_selects_each_provider_default_and_updates_presentation() {
    super::isolated_case(
        "authentication_completion_selects_each_provider_default_and_updates_presentation",
        "child",
        &[
            ("CLOUDFLARE_ACCOUNT_ID", "account"),
            ("CLOUDFLARE_GATEWAY_ID", "gateway"),
        ],
        async |directory| {
            let defaults: Vec<(String, String)> = DEFAULT_MODEL_PER_PROVIDER
                .read()
                .unwrap()
                .iter()
                .map(|(provider, id)| (provider.clone(), id.clone()))
                .collect();
            assert_eq!(defaults.len(), 32, "every configured default is covered");
            for (kind, label) in LABELS {
                let world = started_with(directory, None).await;
                let unknown = placeholder(&world).read().unwrap().clone();
                for (provider, default) in &defaults {
                    register(&world, &format!("decoy-{provider}"), &[default]);
                    register(&world, provider, &["decoy-id", default]);
                    let prior = Arc::new(RwLock::new(unknown.clone()));
                    complete(&world, provider, kind, prior).await.unwrap();
                    assert_eq!(
                        current(&world),
                        format!("{provider}/{default}"),
                        "{provider}"
                    );
                    assert!(notes(&world, &selected(label, default)), "{provider}");
                    assert!(count(&world) > 0.0, "{provider}: counted");
                }
            }
        },
    );
}

/// The registered descriptor of `provider/id`.
fn found(world: &World, provider: &str, id: &str) -> Arc<RwLock<Model>> {
    world
        .host
        .session()
        .model_registry()
        .find(provider, id)
        .unwrap()
}

/// Refresh and availability failures before any selection propagate.
async fn early_failures(directory: &std::path::Path) {
    let world = started_with(directory, None).await;
    refuse_refresh(&world);
    let error = complete(&world, "openai", AuthType::OAuth, placeholder(&world)).await;
    assert_eq!(error.unwrap_err().to_string(), "refresh refused");
    assert!(
        !notes(&world, "Credentials saved"),
        "no status after a refresh failure"
    );

    let world = started_with(directory, None).await;
    refuse_availability(&world);
    let error = complete(&world, "openai", AuthType::OAuth, placeholder(&world)).await;
    assert!(
        error.is_err(),
        "the selection's availability failure propagates"
    );
}

/// An unreadable count becomes zero, and a failing count refresh after a reached selection
/// suppresses the status.
async fn count_failures(directory: &std::path::Path) {
    let default = default_of("openai");
    let world = started_with(directory, None).await;
    register(&world, "openai", &[&default]);
    let known = found(&world, "openai", &default);
    refuse_availability(&world);
    complete(&world, "openai", AuthType::OAuth, known)
        .await
        .unwrap();
    assert_eq!(
        count(&world).to_bits(),
        0f64.to_bits(),
        "unreadable count is zero"
    );
    assert!(notes(&world, &saved("Logged in to Name")));

    let world = started_with(directory, None).await;
    register(&world, "openai", &[&default]);
    refuse_after(&world, 1);
    let error = complete(&world, "openai", AuthType::OAuth, placeholder(&world)).await;
    assert!(error.is_err(), "the count's refresh failure propagates");
    assert_eq!(
        current(&world),
        format!("openai/{default}"),
        "selection was reached"
    );
    assert!(
        !notes(&world, "Credentials saved"),
        "its status is suppressed"
    );
}

/// The count reads the scoped candidates instead of the available ones.
async fn scoped_count(directory: &std::path::Path, scoped: &[&str], expected: f64) {
    let world = started_with(directory, None).await;
    for provider in ["a", "b", "c"] {
        register(&world, provider, &["one"]);
    }
    let references: Vec<_> = scoped
        .iter()
        .map(|reference| ((*reference).to_owned(), None))
        .collect();
    scope(&world, &references);
    let known = found(&world, "a", "one");
    complete(&world, "a", AuthType::ApiKey, known)
        .await
        .unwrap();
    assert_eq!(count(&world).to_bits(), expected.to_bits(), "{scoped:?}");
}

#[test]
fn authentication_completion_preserves_refresh_and_count_failure_boundaries() {
    isolated(
        "authentication_completion_preserves_refresh_and_count_failure_boundaries",
        async |directory| {
            early_failures(directory).await;
            count_failures(directory).await;
            scoped_count(directory, &["a/one", "b/one"], 2.0).await;
            scoped_count(directory, &["a/one"], 1.0).await;
        },
    );
}

#[test]
fn authentication_completion_delegates_subscription_warning_on_the_reached_branch() {
    isolated(
        "authentication_completion_delegates_subscription_warning_on_the_reached_branch",
        async |directory| {
            let world = started_with(directory, None).await;
            sign_in(&world);
            complete(&world, "anthropic", AuthType::OAuth, placeholder(&world))
                .await
                .unwrap();
            assert!(notes(&world, "Selected"), "the default was selected");
            assert!(
                !notes(&world, WARNING),
                "completion does not wait for the notice"
            );
            settle(&world);
            assert!(
                notes(&world, WARNING),
                "the selected model's notice follows"
            );

            let world = started_with(directory, None).await;
            sign_in(&world);
            let default = default_of("anthropic");
            let model = world
                .host
                .session()
                .model_registry()
                .find("anthropic", &default)
                .unwrap();
            world
                .host
                .session()
                .set_model(Arc::clone(&model))
                .await
                .unwrap();
            complete(&world, "anthropic", AuthType::OAuth, model)
                .await
                .unwrap();
            settle(&world);
            assert!(
                notes(&world, WARNING),
                "the session model's notice without a selection"
            );

            let world = started_with(directory, None).await;
            sign_in(&world);
            register(&world, "custom", &["one"]);
            complete(&world, "custom", AuthType::OAuth, placeholder(&world))
                .await
                .unwrap();
            settle(&world);
            assert!(notes(&world, "but no default model is configured"));
            assert!(
                !notes(&world, WARNING),
                "a selection failure shows no notice"
            );
        },
    );
}

/// Waits for `opened`, changes `prior`'s provider when `mutate`, then signs in.
async fn retaining(
    opened: Option<oneshot::Receiver<()>>,
    prior: Arc<RwLock<Model>>,
    mutate: bool,
) -> Result<OAuthCredentials, OAuthError> {
    if let Some(opened) = opened {
        drop(opened.await);
    }
    if mutate {
        prior.write().unwrap().provider = "changed".into();
    }
    Ok(signed("token"))
}

/// Signs in while a callback changes the retained prior descriptor, or while the session
/// slot is replaced, and checks the completion reads the retained descriptor.
async fn retained_case(directory: &std::path::Path, mutate: bool) {
    let default = default_of("openai");
    let world = started_with(directory, None).await;
    let other = register(&world, "other", &["one"]).remove(0);
    let prior = world.host.session().model();
    let (open, opened) = oneshot::channel::<()>();
    let opened = Arc::new(Mutex::new(Some(opened)));
    subscribe(&world, ("openai", "Name"), None, move |_| {
        let opened = opened.lock().unwrap().take();
        Box::pin(retaining(opened, Arc::clone(&prior), mutate))
    });
    register(&world, "openai", &[&default]);
    sign_in_to(&world, AuthType::OAuth, "openai");
    if !mutate {
        world.host.session().set_model(other).await.unwrap();
    }
    open.send(()).unwrap();
    settle(&world);
    if mutate {
        assert!(
            notes(&world, &saved("Logged in to Name")),
            "the read sees the change"
        );
        assert!(!notes(&world, "Selected"));
    } else {
        assert!(notes(&world, &selected("Logged in to Name", &default)));
        assert_eq!(current(&world), format!("openai/{default}"));
    }
}

#[test]
fn authentication_completion_retains_model_handles_across_callbacks() {
    isolated(
        "authentication_completion_retains_model_handles_across_callbacks",
        async |directory| {
            retained_case(directory, true).await;
            retained_case(directory, false).await;
        },
    );
}
