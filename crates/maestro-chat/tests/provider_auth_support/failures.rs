//! Provider failure transactions through retained metadata and input dispatch.
use crate::{manual_runtime::ManualRuntime, selector_failure_support as recorded};
use maestro_app::{AuthStatus, AuthStorage};
use maestro_chat::{
    AuthSelectorProvider, AuthType, OAuthSelectorComponent, OAuthSelectorMode, OAuthSelectorOptions,
};
use maestro_theme::{ColorMode, ColorValue, Theme, ThemeColor, ThemeOptions};
use maestro_tui::Component;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

/// Initial provider inputs and caller-owned state.
pub struct Rig {
    pub storage: Arc<AuthStorage>,
    pub caller: Arc<CallerState>,
    pub events: Rc<RefCell<Vec<Value>>>,
    pub runtime: ManualRuntime,
}
/// Thread-safe storage callback state, separate from terminal callbacks.
#[derive(Default)]
pub struct CallerState {
    pub failing: AtomicBool,
    pub calls: Mutex<Vec<String>>,
    pub failure_id: String,
}
impl Rig {
    /// Own a fallible metadata reader without capturing its owning storage.
    pub fn new(failing: bool, failure_id: &str) -> Self {
        let storage = Arc::new(AuthStorage::in_memory(std::iter::empty().collect()));
        let caller = Arc::new(CallerState {
            failing: AtomicBool::new(failing),
            failure_id: failure_id.into(),
            ..Default::default()
        });
        let state = caller.clone();
        storage.set_fallback_resolver(move |id| {
            state.calls.lock().unwrap().push(id.to_owned());
            if state.failing.load(Ordering::SeqCst) && id == state.failure_id {
                Err(recorded::errors::failure())
            } else {
                Ok(None)
            }
        });
        Self {
            storage,
            caller,
            events: Rc::default(),
            runtime: ManualRuntime::new(),
        }
    }
    /// Construct the real picker using the default or a supplied metadata callback.
    pub fn build(
        &self,
        providers: Vec<AuthSelectorProvider>,
        supplied: bool,
        mode: OAuthSelectorMode,
    ) -> Result<OAuthSelectorComponent, Box<dyn std::error::Error>> {
        let (_, live) = crate::published::published(&Rc::new(
            Theme::new(
                [
                    (ThemeColor::Border, 31),
                    (ThemeColor::Accent, 32),
                    (ThemeColor::Muted, 33),
                    (ThemeColor::Success, 34),
                    (ThemeColor::Error, 35),
                    (ThemeColor::Warning, 36),
                    (ThemeColor::Text, 38),
                ]
                .map(|(role, index)| (role, ColorValue::Index(index))),
                [],
                ColorMode::Truecolor,
                ThemeOptions::default(),
            )
            .unwrap(),
        ));
        let selected = self.events.clone();
        let cancelled = self.events.clone();
        let storage = self.storage.clone();
        OAuthSelectorComponent::new(
            self.storage.clone(),
            providers,
            Rc::new(move |id| selected.borrow_mut().push(json!(["select", id]))),
            Rc::new(move || cancelled.borrow_mut().push(json!(["cancel"]))),
            OAuthSelectorOptions {
                mode,
                theme: live,
                runtime: self.runtime.handle(),
                get_auth_status: supplied.then(|| {
                    Rc::new(move |id: &str| storage.get_auth_status(id).map_err(Into::into))
                        as Rc<dyn Fn(&str) -> Result<AuthStatus, Box<dyn std::error::Error>>>
                }),
            },
        )
    }
    /// Reset observations only after the successful preceding input has completed.
    pub fn enable_failure(&self) {
        self.caller.calls.lock().unwrap().clear();
        self.caller.failing.store(true, Ordering::SeqCst);
    }
    /// Full input error, frames and retained caller events.
    pub fn outcome(
        &self,
        error: &(dyn std::error::Error + 'static),
        before: Option<Vec<String>>,
        partial: Option<Vec<String>>,
    ) -> Value {
        let mut result = json!({"branch":"error", "error":recorded::error_value(error), "events":*self.events.borrow(), "calls":*self.caller.calls.lock().unwrap()});
        if let Some(before) = before {
            result["before"] = json!(before);
        }
        if let Some(partial) = partial {
            result["partial"] = json!(partial);
        }
        result
    }
}
/// Caller-supplied API-key choices in the recorded order.
pub fn providers(choices: &[(&str, &str)]) -> Vec<AuthSelectorProvider> {
    choices
        .iter()
        .map(|(id, name)| AuthSelectorProvider {
            id: (*id).into(),
            name: (*name).into(),
            auth_type: AuthType::ApiKey,
        })
        .collect()
}
/// Constructor and down-navigation failures keep their consuming phase.
pub fn phases() {
    for case in recorded::cases("provider_metadata_errors_keep_their_consuming_phase") {
        let input = case.input;
        let construction = input.phase.as_deref() == Some("construction");
        let rig = Rig::new(construction, "maestro-custom-fallback");
        let result = rig.build(
            providers(&[
                ("maestro-earlier", "Earlier"),
                ("maestro-custom-fallback", "Custom"),
            ]),
            input.supplied.unwrap(),
            OAuthSelectorMode::Login,
        );
        let actual = match result {
            Err(error) => {
                assert!(construction);
                rig.outcome(error.as_ref(), None, None)
            }
            Ok(panel) => {
                assert!(!construction);
                let width = input.width.unwrap();
                let before = panel.render(width);
                rig.enable_failure();
                let error = panel.handle_input("\x1b[B").unwrap_err();
                let partial = panel.render(width);
                panel.handle_input("\r").unwrap();
                panel.handle_input("\x1b").unwrap();
                rig.outcome(error.as_ref(), Some(before), Some(partial))
            }
        };
        assert!(rig.runtime.futures() == 0 && rig.runtime.pending() == 0);
        assert!(actual == case.expected, "recorded provider phase");
    }
}
/// Filtering and up-navigation preserve completed rows and the retained selection.
pub fn rebuilding() {
    for case in recorded::cases("provider_rebuild_failure_keeps_filter_and_navigation_effects") {
        let input = case.input;
        let rig = Rig::new(
            false,
            &format!("maestro-row-{}", input.failure_index.unwrap()),
        );
        let panel = rig
            .build(
                providers(&[
                    ("maestro-row-0", "Choice 0"),
                    ("maestro-row-1", "Choice 1"),
                    ("maestro-row-2", "Choice 2"),
                ]),
                input.supplied.unwrap(),
                OAuthSelectorMode::Login,
            )
            .unwrap();
        let action = input.action.unwrap();
        if action == "up" {
            panel.handle_input("\x1b[B").unwrap();
        }
        let before = panel.render(input.width.unwrap());
        rig.enable_failure();
        let error = panel
            .handle_input(if action == "up" {
                "\x1b[A"
            } else {
                assert_eq!(action, "filter");
                "Choice"
            })
            .unwrap_err();
        let partial = panel.render(input.width.unwrap());
        panel.handle_input("\r").unwrap();
        panel.handle_input("\x1b").unwrap();
        assert!(
            rig.outcome(error.as_ref(), Some(before), Some(partial)) == case.expected,
            "recorded partial rebuild"
        );
        assert!(rig.runtime.futures() == 0 && rig.runtime.pending() == 0);
    }
}
/// Stored records and subscription rows bypass metadata in every input phase.
pub fn bypasses() {
    for case in recorded::cases("provider_stored_and_subscription_paths_skip_failing_metadata") {
        let input = case.input;
        let rig = Rig::new(true, "maestro-custom-fallback");
        let branch = input.branch.unwrap();
        if branch.starts_with("stored") {
            rig.storage.set(
                "maestro-custom-fallback",
                maestro_app::AuthCredential::ApiKey(maestro_app::ApiKeyCredential {
                    env: None,
                    key: String::new(),
                }),
            );
        }
        let mut choices = providers(&[("maestro-custom-fallback", "Custom")]);
        if branch != "stored-match" {
            choices[0].auth_type = AuthType::OAuth;
        }
        let panel = rig
            .build(choices, input.supplied.unwrap(), OAuthSelectorMode::Logout)
            .unwrap();
        let before = panel.render(input.width.unwrap());
        panel.handle_input("\x1b[B").unwrap();
        panel.handle_input("Custom").unwrap();
        let after = panel.render(input.width.unwrap());
        panel.handle_input("\r").unwrap();
        panel.handle_input("\x1b").unwrap();
        let actual = json!({"branch":"ok", "before":before,"after":after,"events":*rig.events.borrow(),"calls":*rig.caller.calls.lock().unwrap()});
        assert!(actual == case.expected, "recorded metadata bypass");
    }
}
