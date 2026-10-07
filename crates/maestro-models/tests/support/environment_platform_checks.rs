use credential_assertions::assert_credential_eq;
#[path = "credential_assertions.rs"]
mod credential_assertions;
use super::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

struct Controlled {
    values: RefCell<HashMap<String, String>>,
    log: RefCell<Vec<String>>,
    exists: Cell<bool>,
    home_value: RefCell<String>,
    home_failure: Cell<bool>,
    join_failure: Cell<bool>,
    scripted: RefCell<HashMap<String, std::collections::VecDeque<Option<String>>>>,
}
impl Default for Controlled {
    fn default() -> Self {
        Self {
            values: RefCell::new(HashMap::new()),
            log: RefCell::new(Vec::new()),
            exists: Cell::new(false),
            home_value: RefCell::new("/home/test".into()),
            home_failure: Cell::new(false),
            join_failure: Cell::new(false),
            scripted: RefCell::new(HashMap::new()),
        }
    }
}
impl Controlled {
    fn put(&self, key: &str, value: &str) {
        self.values.borrow_mut().insert(key.into(), value.into());
    }
}
impl Platform for Controlled {
    fn env(&self, key: &str) -> Result<Option<String>, ThrownValue> {
        self.log.borrow_mut().push(format!("env:{key}"));
        if let Some(values) = self.scripted.borrow_mut().get_mut(key)
            && let Some(value) = values.pop_front()
        {
            return Ok(value);
        }
        Ok(self.values.borrow().get(key).cloned())
    }
    fn home(&self) -> Result<std::path::PathBuf, ThrownValue> {
        self.log.borrow_mut().push("home".into());
        if self.home_failure.get() {
            Err(ThrownValue::Json("home error".into()))
        } else {
            Ok(self.home_value.borrow().clone().into())
        }
    }
    fn join(&self, home: &std::path::Path) -> Result<std::path::PathBuf, ThrownValue> {
        self.log
            .borrow_mut()
            .push(format!("join:{}", home.display()));
        if self.join_failure.get() {
            Err(ThrownValue::Json("join error".into()))
        } else {
            Ok(home.join(".config/gcloud/application_default_credentials.json"))
        }
    }
    fn exists(&self, path: &std::path::Path) -> bool {
        self.log
            .borrow_mut()
            .push(format!("exists:{}", path.display()));
        self.exists.get()
    }
    fn native(&self) -> Result<bool, ThrownValue> {
        Ok(true)
    }
}
fn assert_missing(result: Result<Option<String>, ThrownValue>) {
    match result {
        Err(ThrownValue::Error(e)) => {
            assert_eq!(e.name, "ReferenceError");
            assert_eq!(e.message, "process is not defined");
        }
        _ => panic!("expected ReferenceError"),
    }
}
#[test]
fn browser_process_absence_keeps_reference_error() {
    let p = platform::Browser;
    let state = State::default();
    for provider in [
        "openai",
        "anthropic",
        "github-copilot",
        "google-vertex",
        "toString",
        "__proto__",
    ] {
        assert_missing(get_env_api_key_with_platform(&p, &state, provider));
        assert_missing(find_env_keys_with_platform(&p, provider).map(|v| v.map(|s| s.join(","))));
    }
    assert_missing(get_env_api_key_with_platform(&p, &state, "amazon-bedrock"));
    for provider in ["amazon-bedrock", "ordinary-unknown"] {
        assert!(find_env_keys_with_platform(&p, provider).unwrap().is_none());
    }
    assert!(
        get_env_api_key_with_platform(&p, &state, "ordinary-unknown")
            .unwrap()
            .is_none()
    );
    assert!(state.adc.lock().unwrap().is_none());
}

#[test]
fn environment_rechecks_values_without_key_cache() {
    let p = Controlled::default();
    let state = State::default();
    assert!(find_env_keys_with_platform(&p, "openai").unwrap().is_none());
    p.put("OPENAI_API_KEY", "first");
    let mut names = find_env_keys_with_platform(&p, "openai").unwrap().unwrap();
    names[0] = "changed-name".into();
    assert_eq!(
        find_env_keys_with_platform(&p, "openai").unwrap(),
        Some(vec!["OPENAI_API_KEY".into()])
    );
    assert_credential_eq(
        &(get_env_api_key_with_platform(&p, &state, "openai").unwrap()),
        &(Some("first".into())),
        "environment_rechecks_values_without_key_cache",
    );
    p.put("OPENAI_API_KEY", "second");
    assert_credential_eq(
        &(get_env_api_key_with_platform(&p, &state, "openai").unwrap()),
        &(Some("second".into())),
        "environment_rechecks_values_without_key_cache",
    );
    p.put("OPENAI_API_KEY", "");
    assert!(
        get_env_api_key_with_platform(&p, &state, "openai")
            .unwrap()
            .is_none()
    );
    p.put("GH_TOKEN", "second-token");
    p.scripted.borrow_mut().insert(
        "COPILOT_GITHUB_TOKEN".into(),
        [Some("first-token".into()), None].into(),
    );
    p.log.borrow_mut().clear();
    assert!(
        get_env_api_key_with_platform(&p, &state, "github-copilot")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        *p.log.borrow(),
        [
            "env:COPILOT_GITHUB_TOKEN",
            "env:GH_TOKEN",
            "env:GITHUB_TOKEN",
            "env:COPILOT_GITHUB_TOKEN"
        ]
    );
}

#[test]
fn vertex_short_circuit_order_matches_ambient_lookup() {
    let p = Controlled::default();
    p.put("GOOGLE_CLOUD_API_KEY", "key");
    assert_credential_eq(
        &(get_env_api_key_with_platform(&p, &State::default(), "google-vertex").unwrap()),
        &(Some("key".into())),
        "vertex_short_circuit_order_matches_ambient_lookup",
    );
    assert_eq!(
        *p.log.borrow(),
        ["env:GOOGLE_CLOUD_API_KEY", "env:GOOGLE_CLOUD_API_KEY"]
    );
    let p = Controlled::default();
    p.exists.set(true);
    assert!(
        get_env_api_key_with_platform(&p, &State::default(), "google-vertex")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        *p.log.borrow(),
        [
            "env:GOOGLE_CLOUD_API_KEY",
            "env:GOOGLE_APPLICATION_CREDENTIALS",
            "home",
            "join:/home/test",
            "exists:/home/test/.config/gcloud/application_default_credentials.json",
            "env:GOOGLE_CLOUD_PROJECT",
            "env:GCLOUD_PROJECT",
            "env:GOOGLE_CLOUD_LOCATION"
        ]
    );
    for project in ["GOOGLE_CLOUD_PROJECT", "GCLOUD_PROJECT"] {
        let p = Controlled::default();
        p.put("GOOGLE_APPLICATION_CREDENTIALS", "explicit");
        p.put(project, "p");
        p.put("GOOGLE_CLOUD_LOCATION", "l");
        p.exists.set(true);
        assert_credential_eq(
            &(get_env_api_key_with_platform(&p, &State::default(), "google-vertex").unwrap()),
            &(Some("<authenticated>".into())),
            "vertex_short_circuit_order_matches_ambient_lookup",
        );
        let mut expected = vec![
            "env:GOOGLE_CLOUD_API_KEY",
            "env:GOOGLE_APPLICATION_CREDENTIALS",
            "exists:explicit",
            "env:GOOGLE_CLOUD_PROJECT",
        ];
        if project == "GCLOUD_PROJECT" {
            expected.push("env:GCLOUD_PROJECT");
        }
        expected.push("env:GOOGLE_CLOUD_LOCATION");
        assert_eq!(*p.log.borrow(), expected);
    }
    let p = Controlled::default();
    p.put("GOOGLE_APPLICATION_CREDENTIALS", "missing");
    assert!(
        get_env_api_key_with_platform(&p, &State::default(), "google-vertex")
            .unwrap()
            .is_none()
    );
    assert!(
        !p.log
            .borrow()
            .iter()
            .any(|s| s == "home" || s.starts_with("join:"))
    );
    for join in [false, true] {
        let p = Controlled::default();
        p.home_failure.set(!join);
        p.join_failure.set(join);
        match get_env_api_key_with_platform(&p, &State::default(), "google-vertex") {
            Err(ThrownValue::Json(value)) => {
                assert_eq!(value, if join { "join error" } else { "home error" })
            }
            _ => panic!("expected path lookup failure"),
        }
    }
    let p = Controlled::default();
    p.put("GOOGLE_APPLICATION_CREDENTIALS", "first");
    p.exists.set(true);
    p.put("GOOGLE_CLOUD_PROJECT", "p");
    p.put("GOOGLE_CLOUD_LOCATION", "l");
    let state = State::default();
    assert!(
        get_env_api_key_with_platform(&p, &state, "google-vertex")
            .unwrap()
            .is_some()
    );
    p.put("GOOGLE_APPLICATION_CREDENTIALS", "second");
    p.exists.set(false);
    assert!(
        get_env_api_key_with_platform(&p, &state, "google-vertex")
            .unwrap()
            .is_some()
    );
    assert!(!p.log.borrow().contains(&"exists:second".into()));
}

#[test]
fn bedrock_checks_direct_signals_in_order() {
    let direct_order = [
        "env:AWS_PROFILE",
        "env:AWS_ACCESS_KEY_ID",
        "env:AWS_BEARER_TOKEN_BEDROCK",
        "env:AWS_CONTAINER_CREDENTIALS_RELATIVE_URI",
        "env:AWS_CONTAINER_CREDENTIALS_FULL_URI",
        "env:AWS_WEB_IDENTITY_TOKEN_FILE",
    ];
    let p = Controlled::default();
    assert!(
        get_env_api_key_with_platform(&p, &State::default(), "amazon-bedrock")
            .unwrap()
            .is_none()
    );
    assert_eq!(*p.log.borrow(), direct_order);
    for (key, index) in [
        ("AWS_PROFILE", 0),
        ("AWS_BEARER_TOKEN_BEDROCK", 2),
        ("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI", 3),
        ("AWS_CONTAINER_CREDENTIALS_FULL_URI", 4),
        ("AWS_WEB_IDENTITY_TOKEN_FILE", 5),
    ] {
        let p = Controlled::default();
        p.put(key, "direct");
        assert_credential_eq(
            &(get_env_api_key_with_platform(&p, &State::default(), "amazon-bedrock").unwrap()),
            &(Some("<authenticated>".into())),
            "bedrock_checks_direct_signals_in_order",
        );
        assert_eq!(*p.log.borrow(), direct_order[..=index]);
    }
    let p = Controlled::default();
    p.put("AWS_ACCESS_KEY_ID", "id");
    p.put("AWS_SECRET_ACCESS_KEY", "secret");
    assert!(
        get_env_api_key_with_platform(&p, &State::default(), "amazon-bedrock")
            .unwrap()
            .is_some()
    );
    assert_eq!(
        *p.log.borrow(),
        [
            "env:AWS_PROFILE",
            "env:AWS_ACCESS_KEY_ID",
            "env:AWS_SECRET_ACCESS_KEY"
        ]
    );
}
