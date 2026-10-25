//! Subscription sign-in: the provider's callbacks, their dialog and how the sign-in ends.

use super::super::models::{editor_focused, shows_only};
use super::{
    World, answer, chooser, dialog, editor_restored, isolated, notes, panel_rows, quiet_browser,
    saved_path, settle, sign_in_to, signed, started_with, storage, stored_access, subscribe,
};
use crate::AuthType;
use maestro_app::presentation_data::{OAuthAuthInfo, OAuthPrompt};
use maestro_app::{
    DiagnosticErrorInfo, OAuthCallbacks, OAuthCredentials, OAuthError, OAuthSelectOption,
    OAuthSelectPrompt,
};
use maestro_theme::{ColorMode, Theme, ThemeOptions};
use maestro_tui::Component;
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};
use tokio::sync::oneshot;

/// The authorization link every scripted sign-in shows.
const URL: &str = "https://auth.example/start";

/// Shows the authorization link without instructions.
fn authorize(callbacks: &OAuthCallbacks) -> Result<(), OAuthError> {
    callbacks.on_auth(OAuthAuthInfo {
        url: URL.to_owned(),
        instructions: None,
    })
}

/// A one-shot release a test hands to a script, taken by the script's first sign-in.
#[derive(Clone, Default)]
struct Gate(Arc<Mutex<Option<oneshot::Receiver<()>>>>);

impl Gate {
    /// A closed gate and the sender that opens it.
    fn closed() -> (oneshot::Sender<()>, Self) {
        let (open, opened) = oneshot::channel();
        (open, Self(Arc::new(Mutex::new(Some(opened)))))
    }

    /// Waits until the gate opens.
    async fn passed(&self) {
        let opened = self.0.lock().unwrap().take();
        if let Some(opened) = opened {
            drop(opened.await);
        }
    }
}

/// Values a script records for the test to read.
#[derive(Clone, Default)]
struct Record(Arc<Mutex<Vec<String>>>);

impl Record {
    /// Appends one observation.
    fn push(&self, value: impl Into<String>) {
        self.0.lock().unwrap().push(value.into());
    }

    /// Every observation in order.
    fn all(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

/// A choice option.
fn option(id: &str, label: &str) -> OAuthSelectOption {
    OAuthSelectOption {
        id: id.to_owned(),
        label: label.to_owned(),
    }
}

/// Shows the authorization link with `instructions`, then waits forever.
async fn authorizing(
    callbacks: OAuthCallbacks,
    instructions: Option<String>,
) -> Result<OAuthCredentials, OAuthError> {
    callbacks.on_auth(OAuthAuthInfo {
        url: URL.to_owned(),
        instructions,
    })?;
    std::future::pending().await
}

/// Signs in to a subscription that only shows its link, and checks the dialog's rows.
async fn authorization_case(
    directory: &std::path::Path,
    (id, server): (&str, Option<bool>),
    instructions: Option<&str>,
    (pasted, polls): (bool, bool),
) {
    let world = started_with(directory, None).await;
    let shown = instructions.map(str::to_owned);
    subscribe(&world, (id, "Acme"), server, move |callbacks| {
        Box::pin(authorizing(callbacks, shown.clone()))
    });
    sign_in_to(&world, AuthType::OAuth, id);
    let rows = panel_rows(&world);
    assert!(rows.contains(URL), "{id}: the link is shown");
    if let Some(text) = instructions.filter(|text| !text.is_empty()) {
        assert!(rows.contains(text), "{id}: instructions are shown");
    }
    let paste = "Paste redirect URL below, or complete login in browser:";
    assert_eq!(rows.contains(paste), pasted, "{id} {server:?}");
    let waiting = "Waiting for browser authentication...";
    assert_eq!(rows.contains(waiting), polls, "{id}");
}

#[test]
fn subscription_authorization_keeps_callback_server_and_polling_paths() {
    isolated(
        "subscription_authorization_keeps_callback_server_and_polling_paths",
        async |directory| {
            quiet_browser(directory);
            let cases = [
                (("acme", Some(true)), Some("Use the code"), (true, false)),
                (("acme", Some(false)), Some(""), (false, false)),
                (("acme", None), None, (false, false)),
                (("github-copilot", None), Some("Device code"), (false, true)),
            ];
            for (provider, instructions, shown) in cases {
                authorization_case(directory, provider, instructions, shown).await;
            }
        },
    );
}

/// Races the pasted redirect against the callback server's `code`, recording the second
/// pasted request's answer and whether the dialog was cancelled.
async fn racing_sign_in(
    callbacks: OAuthCallbacks,
    code: Option<oneshot::Receiver<String>>,
    record: Record,
) -> Result<OAuthCredentials, OAuthError> {
    authorize(&callbacks)?;
    let first = callbacks.on_manual_code_input().unwrap();
    let second = callbacks.on_manual_code_input().unwrap();
    let callback = async move {
        match code {
            Some(code) => code.await.ok(),
            None => None,
        }
    };
    let code = tokio::select! {
        manual = first => {
            let manual = manual?;
            record.push(second.await?);
            manual
        }
        Some(code) = callback => code,
    };
    let aborted = callbacks.signal().unwrap().is_aborted();
    record.push(format!("aborted={aborted}"));
    Ok(signed(&code))
}

/// Registers `acme` with a pasted-redirect entry racing a callback server whose result
/// the returned sender supplies.
fn racing(world: &World, record: &Record) -> oneshot::Sender<String> {
    let (server, code) = oneshot::channel::<String>();
    let code = Arc::new(Mutex::new(Some(code)));
    let record = record.clone();
    subscribe(world, ("acme", "Acme"), Some(true), move |callbacks| {
        let code = code.lock().unwrap().take();
        Box::pin(racing_sign_in(callbacks, code, record.clone()))
    });
    server
}

#[test]
fn subscription_manual_input_and_callback_have_controlled_winners() {
    isolated(
        "subscription_manual_input_and_callback_have_controlled_winners",
        async |directory| {
            quiet_browser(directory);
            let world = started_with(directory, None).await;
            let record = Record::default();
            let _server = racing(&world, &record);
            sign_in_to(&world, AuthType::OAuth, "acme");
            answer(&world, "manual-code");
            let stored = stored_access(&world, "acme");
            assert_eq!(stored.as_deref(), Some("manual-code"));
            let both = ["manual-code", "aborted=false"];
            assert_eq!(record.all(), both, "both requests see one answer");
            assert!(editor_restored(&world));

            let world = started_with(directory, None).await;
            let server = racing(&world, &Record::default());
            sign_in_to(&world, AuthType::OAuth, "acme");
            answer(&world, "");
            assert!(stored_access(&world, "acme").is_none(), "empty text waits");
            assert!(!editor_restored(&world), "the dialog stays");
            server.send("server-code".to_owned()).unwrap();
            settle(&world);
            let stored = stored_access(&world, "acme");
            assert_eq!(stored.as_deref(), Some("server-code"));
            assert!(
                editor_restored(&world),
                "the finished sign-in releases the dialog"
            );

            let world = started_with(directory, None).await;
            let record = Record::default();
            let _server = racing(&world, &record);
            sign_in_to(&world, AuthType::OAuth, "acme");
            world.terminal.send_input("\x1b");
            settle(&world);
            assert!(stored_access(&world, "acme").is_none());
            assert!(editor_restored(&world), "cancelling puts the editor back");
            assert!(!notes(&world, "Error: "), "cancelling is silent");
            assert!(
                record.all().is_empty(),
                "the cancelled entry ends the sign-in"
            );
        },
    );
}

#[test]
fn subscription_blank_manual_input_keeps_submission_and_escape_working() {
    isolated(
        "subscription_blank_manual_input_keeps_submission_and_escape_working",
        async |directory| {
            quiet_browser(directory);
            for cancel in [true, false] {
                let world = started_with(directory, None).await;
                let record = Record::default();
                let _server = racing(&world, &record);
                sign_in_to(&world, AuthType::OAuth, "acme");
                answer(&world, "");
                assert!(!editor_restored(&world), "blank input keeps the dialog");
                if cancel {
                    world.terminal.send_input("\x1b");
                    settle(&world);
                    assert!(stored_access(&world, "acme").is_none());
                    assert!(!notes(&world, "Error: "), "cancellation is silent");
                    assert!(record.all().is_empty());
                } else {
                    answer(&world, "redirect-code");
                    assert_eq!(
                        stored_access(&world, "acme").as_deref(),
                        Some("redirect-code")
                    );
                    assert_eq!(record.all(), ["redirect-code", "aborted=false"]);
                }
                assert!(
                    editor_restored(&world),
                    "the sign-in finishes after blank input"
                );
            }
        },
    );
}

/// Shows progress, asks three questions and one choice, recording each answer, then waits
/// for `gate` before signing in.
async fn questioning(
    callbacks: OAuthCallbacks,
    record: Record,
    gate: Gate,
) -> Result<OAuthCredentials, OAuthError> {
    callbacks.on_progress("Step one")?;
    callbacks.on_progress("Step two")?;
    let questions = [
        ("Code?", Some("1234")),
        ("Optional?", Some("")),
        ("Bare?", None),
    ];
    for (message, placeholder) in questions {
        let prompt = OAuthPrompt {
            message: message.to_owned(),
            placeholder: placeholder.map(str::to_owned),
            allow_empty: Some(false),
        };
        record.push(callbacks.on_prompt(prompt).await?);
    }
    let choice = OAuthSelectPrompt {
        message: "Pick an account".to_owned(),
        options: vec![option("a", "First"), option("b", "Second")],
    };
    let chosen = callbacks.on_select(choice).unwrap().await?;
    record.push(chosen.unwrap_or_default());
    gate.passed().await;
    Ok(signed("done"))
}

#[test]
fn subscription_prompt_progress_and_selection_reach_the_retained_dialog() {
    isolated(
        "subscription_prompt_progress_and_selection_reach_the_retained_dialog",
        async |directory| {
            let world = started_with(directory, None).await;
            let record = Record::default();
            let (open, gate) = Gate::closed();
            let script_record = record.clone();
            subscribe(&world, ("acme", "Acme"), None, move |callbacks| {
                Box::pin(questioning(callbacks, script_record.clone(), gate.clone()))
            });
            sign_in_to(&world, AuthType::OAuth, "acme");
            let shown: Rc<dyn Component> = dialog(&world);
            let rows = panel_rows(&world);
            let (one, two) = (rows.find("Step one"), rows.find("Step two"));
            assert!(one.unwrap() < two.unwrap(), "progress keeps its order");
            assert!(rows.contains("Code?") && rows.contains("e.g., 1234"));
            answer(&world, "abc");
            let rows = panel_rows(&world);
            assert!(rows.contains("Optional?"));
            assert_eq!(rows.matches("e.g.,").count(), 1, "an empty hint is hidden");
            answer(&world, "");
            answer(&world, "x");
            let choice = chooser(&world).expect("the choice replaces the dialog");
            assert!(
                choice
                    .render(super::WIDE)
                    .concat()
                    .contains("Pick an account")
            );
            world.terminal.send_input("\x1b[B");
            world.terminal.send_input("\r");
            settle(&world);
            assert!(shows_only(&world, &shown), "the same dialog comes back");
            assert_eq!(
                record.all(),
                ["abc", "", "x", "b"],
                "answers and the chosen id"
            );
            open.send(()).unwrap();
            settle(&world);
            assert_eq!(stored_access(&world, "acme").as_deref(), Some("done"));
        },
    );
}

/// Shows progress, waits for `gate`, then asks to choose among `options` and records the
/// answer.
async fn choosing(
    callbacks: OAuthCallbacks,
    options: Vec<OAuthSelectOption>,
    (record, gate): (Record, Gate),
) -> Result<OAuthCredentials, OAuthError> {
    callbacks.on_progress("Choosing")?;
    gate.passed().await;
    let prompt = OAuthSelectPrompt {
        message: "Pick".to_owned(),
        options,
    };
    let chosen = callbacks.on_select(prompt).unwrap().await?;
    record.push(format!("{chosen:?}"));
    std::future::pending().await
}

/// Answers a choice among `options` with `keys`, expecting the identifier `expected`.
async fn choice_case(
    directory: &std::path::Path,
    options: Vec<OAuthSelectOption>,
    keys: &[&str],
    expected: Option<&str>,
) {
    let world = started_with(directory, None).await;
    let record = Record::default();
    let (open, gate) = Gate::closed();
    let (script_record, offered) = (record.clone(), options.clone());
    subscribe(&world, ("acme", "Acme"), None, move |callbacks| {
        let answer = (script_record.clone(), gate.clone());
        Box::pin(choosing(callbacks, offered.clone(), answer))
    });
    sign_in_to(&world, AuthType::OAuth, "acme");
    let shown: Rc<dyn Component> = dialog(&world);
    open.send(()).unwrap();
    settle(&world);
    assert!(
        chooser(&world).is_some(),
        "{options:?}: the choice is shown"
    );
    for key in keys {
        world.terminal.send_input(key);
    }
    settle(&world);
    let id = expected.map(str::to_owned);
    assert_eq!(record.all(), [format!("{id:?}")], "{options:?}");
    assert!(
        shows_only(&world, &shown),
        "{options:?}: the dialog comes back"
    );
}

#[test]
fn subscription_option_selection_preserves_ids_and_dialog_identity() {
    isolated(
        "subscription_option_selection_preserves_ids_and_dialog_identity",
        async |directory| {
            let (down, enter, cancel) = ("\x1b[B", "\r", "\x1b");
            let cases: Vec<(Vec<OAuthSelectOption>, Vec<&str>, Option<&str>)> = vec![
                (
                    vec![option("a", "First"), option("b", "Second")],
                    vec![down, enter],
                    Some("b"),
                ),
                (
                    vec![option("x", "A"), option("x", "B")],
                    vec![down, enter],
                    Some("x"),
                ),
                (
                    vec![option("a", "Same"), option("b", "Same")],
                    vec![down, enter],
                    Some("a"),
                ),
                (vec![option("e", "")], vec![enter, cancel], None),
                (vec![], vec![enter, cancel], None),
                (vec![option("a", "First")], vec![cancel], None),
            ];
            for (options, keys, expected) in cases {
                choice_case(directory, options, &keys, expected).await;
            }
        },
    );
}

/// Signs in to a subscription that ends with `outcome` at once, checking what is shown.
async fn finish_case(
    directory: &std::path::Path,
    outcome: Result<&str, &str>,
    shown: Option<&str>,
) {
    let world = started_with(directory, None).await;
    let result = outcome.map(str::to_owned).map_err(str::to_owned);
    subscribe(&world, ("acme", "Acme"), None, move |_| {
        let result = result.clone().map(|access| signed(&access));
        let result = result.map_err(|message| DiagnosticErrorInfo::error(message).into());
        Box::pin(std::future::ready(result))
    });
    sign_in_to(&world, AuthType::OAuth, "acme");
    assert!(editor_restored(&world), "{outcome:?}");
    assert!(editor_focused(&world), "{outcome:?}");
    let stored = stored_access(&world, "acme");
    assert_eq!(
        stored.as_deref(),
        outcome.ok(),
        "only success stores credentials"
    );
    match shown {
        Some(text) => assert!(notes(&world, text), "{text}"),
        None => assert!(!notes(&world, "Error: "), "cancelling is silent"),
    }
    if outcome.is_ok() {
        assert!(notes(&world, &saved_path()));
    }
}

#[test]
fn subscription_finish_restores_editor_on_success_cancel_and_failure() {
    isolated(
        "subscription_finish_restores_editor_on_success_cancel_and_failure",
        async |directory| {
            let failed = "Error: Failed to login to Acme: access denied";
            finish_case(
                directory,
                Ok("token"),
                Some("Logged in to Acme. Credentials saved to"),
            )
            .await;
            finish_case(directory, Err("access denied"), Some(failed)).await;
            finish_case(directory, Err("Login cancelled"), None).await;
        },
    );
}

/// Shows progress, waits for `gate`, then signs in.
async fn gated(callbacks: OAuthCallbacks, gate: Gate) -> Result<OAuthCredentials, OAuthError> {
    callbacks.on_progress("Working")?;
    gate.passed().await;
    Ok(signed("kept"))
}

#[test]
fn subscription_callbacks_release_without_sender_drop() {
    isolated(
        "subscription_callbacks_release_without_sender_drop",
        async |directory| {
            let world = started_with(directory, None).await;
            let retained: Arc<Mutex<Option<OAuthCallbacks>>> = Arc::default();
            let (open, gate) = Gate::closed();
            let kept = Arc::clone(&retained);
            subscribe(&world, ("acme", "Acme"), None, move |callbacks| {
                *kept.lock().unwrap() = Some(Arc::clone(&callbacks));
                Box::pin(gated(callbacks, gate.clone()))
            });
            sign_in_to(&world, AuthType::OAuth, "acme");
            let shown = Rc::downgrade(&dialog(&world));
            open.send(()).unwrap();
            settle(&world);
            assert_eq!(stored_access(&world, "acme").as_deref(), Some("kept"));
            assert!(
                editor_restored(&world),
                "a retained alias does not hold the sign-in"
            );
            assert!(shown.upgrade().is_none(), "the dialog is released");
            let late = retained.lock().unwrap().clone().unwrap();
            let refused = late.on_progress("late").unwrap_err();
            assert_eq!(refused.to_string(), "Local dispatch is closed");
            assert!(editor_restored(&world), "a late callback paints nothing");
        },
    );
}

/// Where a scripted callback runs.
#[derive(Clone, Copy, Debug)]
enum Route {
    /// The sign-in's own future, on the terminal's thread.
    Owner,
    /// A task spawned on the terminal's executor.
    Spawned,
    /// Another operating-system thread.
    Foreign,
}

/// Calls `on_progress` from `route`, returning its outcome.
async fn progress_from(callbacks: OAuthCallbacks, route: Route) -> Result<(), OAuthError> {
    match route {
        Route::Owner => callbacks.on_progress("Working"),
        Route::Spawned => tokio::task::spawn_local(async move { callbacks.on_progress("Working") })
            .await
            .unwrap(),
        Route::Foreign => {
            let (done, finished) = oneshot::channel();
            let thread = std::thread::spawn(move || {
                drop(done.send(callbacks.on_progress("Working")));
            });
            let outcome = finished.await.unwrap();
            thread.join().unwrap();
            outcome
        }
    }
}

/// Drives the terminal runtime and its executor until `record` holds `count` entries.
async fn until_recorded(world: &World, record: &Record, count: usize) {
    while record.all().len() < count {
        drop(world.runtime.drain_futures());
        tokio::task::yield_now().await;
    }
}

/// The gates and observations of one failing presentation.
#[derive(Clone)]
struct Failing {
    /// Records the failure and whether credentials were already saved.
    record: Record,
    /// Opens when the presentation is broken.
    broken: Gate,
    /// Opens when the presentation works again.
    repaired: Gate,
    /// The store the sign-in saves into.
    storage: Arc<maestro_app::AuthStorage>,
}

/// Fails one presentation `phase` from `route`, recording the failure and whether the
/// store already holds credentials, then returns the original failure.
async fn failing_sign_in(
    callbacks: OAuthCallbacks,
    (phase, route): (&'static str, Route),
    failing: Failing,
) -> Result<OAuthCredentials, OAuthError> {
    if phase == "progress" {
        callbacks.on_progress("Ready")?;
    }
    failing.broken.passed().await;
    let outcome = if phase == "auth" {
        authorize(&callbacks)
    } else {
        progress_from(Arc::clone(&callbacks), route).await
    };
    let error = outcome.expect_err("the presentation fails");
    failing.record.push(error.to_string());
    let stored = failing.storage.get("acme").is_some();
    failing.record.push(format!("stored={stored}"));
    failing.repaired.passed().await;
    Err(error)
}

/// Breaks the published theme while `phase` is shown from `route`, then checks the
/// original failure ends the sign-in without saving.
async fn failure_case(directory: &std::path::Path, phase: &'static str, route: Route) {
    let world = started_with(directory, None).await;
    let (fail, broken) = Gate::closed();
    let (finish, repaired) = Gate::closed();
    let observed = Failing {
        record: Record::default(),
        broken,
        repaired,
        storage: storage(&world),
    };
    let script = observed.clone();
    subscribe(&world, ("acme", "Acme"), None, move |callbacks| {
        Box::pin(failing_sign_in(callbacks, (phase, route), script.clone()))
    });
    sign_in_to(&world, AuthType::OAuth, "acme");
    let theme = world.themes.theme().get().unwrap();
    let empty = Theme::new([], [], ColorMode::Truecolor, ThemeOptions::default()).unwrap();
    world.themes.set_theme_instance(Rc::new(empty)).unwrap();
    fail.send(()).unwrap();
    until_recorded(&world, &observed.record, 2).await;
    world.themes.set_theme_instance(theme).unwrap();
    let recorded = observed.record.all();
    assert_eq!(
        recorded[1], "stored=false",
        "{phase} {route:?}: nothing saved first"
    );
    finish.send(()).unwrap();
    settle(&world);
    let shown = format!("Error: Failed to login to Acme: {}", recorded[0]);
    assert!(
        notes(&world, &shown),
        "{phase} {route:?}: the original failure"
    );
    assert!(!notes(&world, "Login cancelled"));
    assert!(stored_access(&world, "acme").is_none());
    assert!(editor_restored(&world));
}

#[test]
fn subscription_presentation_failure_keeps_original_diagnostic() {
    isolated(
        "subscription_presentation_failure_keeps_original_diagnostic",
        async |directory| {
            failure_case(directory, "auth", Route::Owner).await;
            for route in [Route::Owner, Route::Spawned, Route::Foreign] {
                failure_case(directory, "progress", route).await;
            }
        },
    );
}

/// Waits for `gate`, then signs in.
async fn later(gate: Gate) -> Result<OAuthCredentials, OAuthError> {
    gate.passed().await;
    Ok(signed("later"))
}

#[test]
fn login_callbacks_do_not_retain_the_mode() {
    isolated(
        "login_callbacks_do_not_retain_the_mode",
        async |directory| {
            let world = started_with(directory, None).await;
            super::super::models::submit_held(&world, "/login");
            assert!(chooser(&world).is_some());
            let mode = Rc::downgrade(&world.mode);
            let World { mode: owned, .. } = world;
            drop(owned);
            assert!(mode.upgrade().is_none(), "the method choice keeps no mode");

            let world = started_with(directory, None).await;
            let (open, gate) = Gate::closed();
            subscribe(&world, ("acme", "Acme"), None, move |_| {
                Box::pin(later(gate.clone()))
            });
            sign_in_to(&world, AuthType::OAuth, "acme");
            let mode = Rc::downgrade(&world.mode);
            let World {
                mode: owned,
                runtime,
                ..
            } = world;
            drop(owned);
            assert!(
                mode.upgrade().is_some(),
                "the admitted sign-in keeps the mode"
            );
            open.send(()).unwrap();
            drop(runtime.drain_futures());
            assert!(
                mode.upgrade().is_none(),
                "the settled sign-in releases the mode"
            );
        },
    );
}
