use credential_assertions::assert_credential_eq;
#[path = "credential_assertions.rs"]
mod credential_assertions;
use super::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

struct Controlled {
    values: RefCell<HashMap<String, String>>,
    log: RefCell<Vec<String>>,
    bun: Cell<bool>,
    native: Cell<bool>,
    own: Cell<usize>,
    bytes: RefCell<Option<Vec<u8>>>,
    reads: Cell<usize>,
    recovery_checks: Cell<usize>,
    facilities: Cell<[bool; 3]>,
    exists: Cell<bool>,
    home_value: RefCell<String>,
    windows: Cell<bool>,
    home_failure: Cell<bool>,
    join_failure: Cell<bool>,
    process: Cell<bool>,
    substitute_env: Cell<bool>,
    host_values: RefCell<HashMap<String, (bool, ThrownValue)>>,
    failures: RefCell<HashMap<String, platform::HostThrown>>,
    scripted: RefCell<HashMap<String, std::collections::VecDeque<Option<String>>>>,
}
impl Default for Controlled {
    fn default() -> Self {
        Self {
            values: RefCell::new(HashMap::new()),
            log: RefCell::new(Vec::new()),
            bun: Cell::new(false),
            native: Cell::new(true),
            own: Cell::new(0),
            bytes: RefCell::new(None),
            reads: Cell::new(0),
            recovery_checks: Cell::new(0),
            facilities: Cell::new([true; 3]),
            exists: Cell::new(false),
            home_value: RefCell::new("/home/test".into()),
            windows: Cell::new(false),
            home_failure: Cell::new(false),
            join_failure: Cell::new(false),
            process: Cell::new(true),
            substitute_env: Cell::new(false),
            host_values: RefCell::new(HashMap::new()),
            failures: RefCell::new(HashMap::new()),
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
    fn env(&self, key: &str) -> Result<Option<EnvironmentValue>, ThrownValue> {
        if !self.process.get() && !self.substitute_env.get() {
            return Err(missing_process());
        }
        self.log.borrow_mut().push(format!("env:{key}"));
        if let Some(value) = self.failures.borrow().get(key) {
            return Err(platform::host_thrown(value.clone()).unwrap());
        }
        if let Some((truthy, text)) = self.host_values.borrow().get(key) {
            return Ok(Some(platform::host_environment_value(
                *truthy,
                text.clone(),
            )));
        }
        if let Some(values) = self.scripted.borrow_mut().get_mut(key)
            && let Some(value) = values.pop_front()
        {
            return Ok(value.map(EnvironmentValue::string));
        }
        Ok(self
            .values
            .borrow()
            .get(key)
            .cloned()
            .map(EnvironmentValue::string))
    }
    fn home(&self) -> Result<String, ThrownValue> {
        self.log.borrow_mut().push("home".into());
        if self.home_failure.get() {
            Err(ThrownValue::Json("home error".into()))
        } else {
            Ok(self.home_value.borrow().clone())
        }
    }
    fn join(&self, home: &str) -> Result<String, ThrownValue> {
        self.log.borrow_mut().push(format!("join:{home}"));
        if self.join_failure.get() {
            Err(ThrownValue::Json("join error".into()))
        } else {
            Ok(if self.windows.get() {
                platform::join_windows(home)
            } else {
                platform::join_posix(home)
            })
        }
    }
    fn exists(&self, path: &str) -> bool {
        self.log.borrow_mut().push(format!("exists:{path}"));
        self.exists.get()
    }
    fn facilities(&self) -> [bool; 3] {
        self.facilities.get()
    }
    fn native(&self) -> Result<bool, ThrownValue> {
        Ok(self.native.get())
    }
    fn bun(&self) -> Result<bool, ThrownValue> {
        self.recovery_checks.set(self.recovery_checks.get() + 1);
        if !self.process.get() {
            Err(missing_process())
        } else {
            Ok(self.bun.get())
        }
    }
    fn own_count(&self) -> Result<usize, ThrownValue> {
        Ok(self.own.get())
    }
    fn proc_bytes(&self) -> Option<Vec<u8>> {
        self.reads.set(self.reads.get() + 1);
        self.bytes.borrow().clone()
    }
}
#[test]
fn proc_recovery_requires_bun_and_empty_environment() {
    for bun in [false, true] {
        for own in [0, 1] {
            let p = Controlled::default();
            p.bun.set(bun);
            p.own.set(own);
            *p.bytes.borrow_mut() = Some(b"OPENAI_API_KEY=recovered\0".to_vec());
            let state = State::default();
            assert_credential_eq(
                &(get_env_api_key_with_platform(&p, &state, "openai").unwrap()),
                &((bun && own == 0).then(|| "recovered".into())),
                "proc_recovery_requires_bun_and_empty_environment",
            );
            assert_eq!(p.reads.get(), usize::from(bun && own == 0));
        }
    }
    let p = Controlled::default();
    p.bun.set(true);
    p.put("OPENAI_API_KEY", "direct");
    assert_credential_eq(
        &(get_env_api_key_with_platform(&p, &State::default(), "openai").unwrap()),
        &(Some("direct".into())),
        "proc_recovery_requires_bun_and_empty_environment",
    );
    assert_eq!(p.reads.get(), 0);
    let p = Controlled::default();
    p.native.set(false);
    p.bun.set(false);
    assert!(
        get_env_api_key_with_platform(&p, &State::default(), "openai")
            .unwrap()
            .is_none()
    );
    assert_eq!(p.reads.get(), 0);
}
const INHERITED: &[(&str, &str)] = &[
    ("constructor", "function Object() { [native code] }"),
    (
        "__defineGetter__",
        "function __defineGetter__() { [native code] }",
    ),
    (
        "__defineSetter__",
        "function __defineSetter__() { [native code] }",
    ),
    (
        "hasOwnProperty",
        "function hasOwnProperty() { [native code] }",
    ),
    (
        "__lookupGetter__",
        "function __lookupGetter__() { [native code] }",
    ),
    (
        "__lookupSetter__",
        "function __lookupSetter__() { [native code] }",
    ),
    (
        "isPrototypeOf",
        "function isPrototypeOf() { [native code] }",
    ),
    (
        "propertyIsEnumerable",
        "function propertyIsEnumerable() { [native code] }",
    ),
    ("toString", "function toString() { [native code] }"),
    ("valueOf", "function valueOf() { [native code] }"),
    ("__proto__", "[object Object]"),
    (
        "toLocaleString",
        "function toLocaleString() { [native code] }",
    ),
];

#[test]
fn proc_entries_split_nul_and_first_equals() {
    let p = Controlled::default();
    p.bun.set(true);
    let mut bytes = b"OPENAI_API_KEY=first\0OPENAI_API_KEY=last=equals\0EMPTY=\0noequal\0=leading\0\0BOM=\xef\xbb\xbfvalue\0BAD=\xff\0UNICODE=\xe9\x9b\xaa\0\xef\xbb\xbfNAME=bom-name\0".to_vec();
    for &(_, key) in INHERITED {
        bytes.extend_from_slice(format!("{key}=controlled-key\0").as_bytes());
    }
    bytes.extend_from_slice(b"FINAL=no-final-nul");
    *p.bytes.borrow_mut() = Some(bytes);
    let state = State::default();
    for &(provider, _) in INHERITED {
        assert!(
            find_env_keys_with_platform(&p, &state, provider)
                .unwrap()
                .is_none()
        );
        assert!(
            get_env_api_key_with_platform(&p, &state, provider)
                .unwrap()
                .is_none()
        );
    }
    assert_credential_eq(
        &(get_env_api_key_with_platform(&p, &state, "openai").unwrap()),
        &(Some("last=equals".into())),
        "proc_entries_split_nul_and_first_equals",
    );
    for (key, value) in [
        ("EMPTY", None),
        ("noequal", None),
        ("", None),
        ("BOM", Some("\u{feff}value")),
        ("BAD", Some("\u{fffd}")),
        ("UNICODE", Some("雪")),
        ("\u{feff}NAME", Some("bom-name")),
        ("FINAL", Some("no-final-nul")),
    ] {
        assert_credential_eq(
            &(get_proc_env(&p, &state, key).unwrap().as_deref()),
            &(value),
            "proc_entries_split_nul_and_first_equals",
        );
    }
    assert_eq!(p.reads.get(), 1);
}
#[test]
fn proc_read_failure_keeps_empty_cache() {
    for content in [
        None,
        Some(b"OPENAI_API_KEY=original\0GROQ_API_KEY=other\0".to_vec()),
    ] {
        let p = Controlled::default();
        p.bun.set(true);
        *p.bytes.borrow_mut() = content;
        let state = State::default();
        let first = get_env_api_key_with_platform(&p, &state, "openai").unwrap();
        *p.bytes.borrow_mut() = Some(b"OPENAI_API_KEY=changed\0GROQ_API_KEY=changed\0".to_vec());
        assert_credential_eq(
            &(get_env_api_key_with_platform(&p, &state, "openai").unwrap()),
            &(first),
            "proc_read_failure_keeps_empty_cache",
        );
        assert_credential_eq(
            &(get_env_api_key_with_platform(&p, &state, "groq").unwrap()),
            &(first.map(|_| "other".into())),
            "proc_read_failure_keeps_empty_cache",
        );
        assert_eq!(p.reads.get(), 1);
    }
}

#[test]
fn native_module_readiness_retries_adc_probe() {
    for missing in 0..3 {
        for exists in [false, true] {
            let p = Controlled::default();
            p.exists.set(exists);
            p.put("GOOGLE_CLOUD_PROJECT", "p");
            p.put("GOOGLE_CLOUD_LOCATION", "l");
            let mut facilities = [true; 3];
            facilities[missing] = false;
            p.facilities.set(facilities);
            let state = State::default();
            assert!(
                get_env_api_key_with_platform(&p, &state, "google-vertex")
                    .unwrap()
                    .is_none()
            );
            assert!(!p.log.borrow().iter().any(|s| s.starts_with("exists:")));
            p.facilities.set([true; 3]);
            let expected = exists.then(|| "<authenticated>".to_owned());
            assert_credential_eq(
                &(get_env_api_key_with_platform(&p, &state, "google-vertex").unwrap()),
                &(expected),
                "native_module_readiness_retries_adc_probe",
            );
            p.exists.set(!exists);
            assert_credential_eq(
                &(get_env_api_key_with_platform(&p, &state, "google-vertex").unwrap()),
                &(expected),
                "native_module_readiness_retries_adc_probe",
            );
            assert_eq!(
                p.log
                    .borrow()
                    .iter()
                    .filter(|s| s.starts_with("exists:"))
                    .count(),
                1
            );
        }
    }
}

fn missing_process() -> ThrownValue {
    ThrownValue::Error(Box::new(crate::Error {
        name: "ReferenceError".into(),
        message: "process is not defined".into(),
        stack: None,
        code: None,
        errno: None,
        cause: None,
    }))
}
fn expect_host_failure(result: Result<Option<String>, ThrownValue>) -> ThrownValue {
    match result {
        Err(failure) => failure,
        Ok(_) => panic!("expected host failure"),
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
    for substitute in [false, true] {
        let p = Controlled::default();
        p.process.set(false);
        p.native.set(false);
        p.facilities.set([false; 3]);
        p.substitute_env.set(substitute);
        let state = State::default();
        for provider in ["openai", "google-vertex"] {
            assert_missing(get_env_api_key_with_platform(&p, &state, provider));
            assert_missing(
                find_env_keys_with_platform(&p, &state, provider).map(|v| v.map(|s| s.join(","))),
            );
        }
        assert_missing(get_env_api_key_with_platform(&p, &state, "amazon-bedrock"));
        assert!(
            find_env_keys_with_platform(&p, &state, "amazon-bedrock")
                .unwrap()
                .is_none()
        );
        assert!(
            find_env_keys_with_platform(&p, &state, "ordinary-unknown")
                .unwrap()
                .is_none()
        );
        assert!(
            get_env_api_key_with_platform(&p, &state, "ordinary-unknown")
                .unwrap()
                .is_none()
        );
    }
}
#[test]
fn browser_shim_has_no_native_credential_files() {
    let p = Controlled::default();
    p.native.set(false);
    p.facilities.set([false; 3]);
    let state = State::default();
    assert!(
        get_env_api_key_with_platform(&p, &state, "openai")
            .unwrap()
            .is_none()
    );
    p.put("OPENAI_API_KEY", "direct");
    assert_credential_eq(
        &(get_env_api_key_with_platform(&p, &state, "openai").unwrap()),
        &(Some("direct".into())),
        "browser_shim_has_no_native_credential_files",
    );
    p.put("GOOGLE_CLOUD_PROJECT", "p");
    p.put("GOOGLE_CLOUD_LOCATION", "l");
    assert!(
        get_env_api_key_with_platform(&p, &state, "google-vertex")
            .unwrap()
            .is_none()
    );
    p.facilities.set([true; 3]);
    p.exists.set(true);
    assert!(
        get_env_api_key_with_platform(&p, &state, "google-vertex")
            .unwrap()
            .is_none()
    );
    assert!(!p.log.borrow().iter().any(|s| s.starts_with("exists:")));
    assert_eq!(p.reads.get(), 0);
}

#[test]
fn environment_rechecks_values_without_key_cache() {
    let p = Controlled::default();
    let state = State::default();
    assert!(
        find_env_keys_with_platform(&p, &state, "openai")
            .unwrap()
            .is_none()
    );
    p.put("OPENAI_API_KEY", "first");
    let mut names = find_env_keys_with_platform(&p, &state, "openai")
        .unwrap()
        .unwrap();
    names[0] = "changed-name".into();
    assert_eq!(
        find_env_keys_with_platform(&p, &state, "openai").unwrap(),
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
    for project in ["GOOGLE_CLOUD_PROJECT", "GCLOUD_PROJECT"] {
        for mask in 0..8 {
            let p = Controlled::default();
            p.bun.set(true);
            p.exists.set(mask & 1 != 0);
            *p.bytes.borrow_mut() = Some(format!("GOOGLE_APPLICATION_CREDENTIALS=recovered-path\0{project}={}\0GOOGLE_CLOUD_LOCATION={}\0", if mask & 2 != 0 { "p" } else { "" }, if mask & 4 != 0 { "l" } else { "" }).into_bytes());
            assert_credential_eq(
                &(get_env_api_key_with_platform(&p, &State::default(), "google-vertex").unwrap()),
                &((mask == 7).then(|| "<authenticated>".into())),
                "vertex_short_circuit_order_matches_ambient_lookup",
            );
            assert!(p.log.borrow().contains(&"exists:recovered-path".into()));
            assert!(!p.log.borrow().contains(&"home".into()));
            assert_eq!(p.reads.get(), 1);
        }
    }
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
fn adc_join_matches_posix_and_windows_paths() {
    for (windows, home, expected) in [
        (
            false,
            "/home/test",
            "/home/test/.config/gcloud/application_default_credentials.json",
        ),
        (
            false,
            "/tmp/a/../b//",
            "/tmp/b/.config/gcloud/application_default_credentials.json",
        ),
        (
            false,
            "relative/./home",
            "relative/home/.config/gcloud/application_default_credentials.json",
        ),
        (
            false,
            "",
            ".config/gcloud/application_default_credentials.json",
        ),
        (
            false,
            "/",
            "/.config/gcloud/application_default_credentials.json",
        ),
        (
            false,
            "~",
            "~/.config/gcloud/application_default_credentials.json",
        ),
        (
            true,
            r"C:\Users\test",
            r"C:\Users\test\.config\gcloud\application_default_credentials.json",
        ),
        (
            true,
            r"C:\a\..\b\",
            r"C:\b\.config\gcloud\application_default_credentials.json",
        ),
        (
            true,
            "C:relative",
            r"C:relative\.config\gcloud\application_default_credentials.json",
        ),
        (
            true,
            r"\\server\share\a\..\b",
            r"\\server\share\b\.config\gcloud\application_default_credentials.json",
        ),
        (
            true,
            "",
            r".config\gcloud\application_default_credentials.json",
        ),
        (
            true,
            r"\",
            r"\.config\gcloud\application_default_credentials.json",
        ),
        (
            false,
            "/\u{feff}/\u{85}",
            "/\u{feff}/\u{85}/.config/gcloud/application_default_credentials.json",
        ),
        (
            true,
            "C:\\\u{feff}\\\u{85}",
            "C:\\\u{feff}\\\u{85}\\.config\\gcloud\\application_default_credentials.json",
        ),
    ] {
        let p = Controlled::default();
        p.windows.set(windows);
        *p.home_value.borrow_mut() = home.into();
        p.put("GOOGLE_CLOUD_PROJECT", "p");
        p.put("GOOGLE_CLOUD_LOCATION", "l");
        assert!(
            get_env_api_key_with_platform(&p, &State::default(), "google-vertex")
                .unwrap()
                .is_none()
        );
        assert!(
            p.log.borrow().contains(&format!("exists:{expected}")),
            "home={home:?}, log={:?}",
            p.log.borrow()
        );
    }
}

#[test]
fn bedrock_checks_direct_signals_before_proc_signals() {
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
    assert_eq!(p.recovery_checks.get(), 6);
    for (key, index) in [
        ("AWS_PROFILE", 0),
        ("AWS_BEARER_TOKEN_BEDROCK", 2),
        ("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI", 3),
        ("AWS_CONTAINER_CREDENTIALS_FULL_URI", 4),
        ("AWS_WEB_IDENTITY_TOKEN_FILE", 5),
    ] {
        let p = Controlled::default();
        p.bun.set(true);
        p.put(key, "direct");
        *p.bytes.borrow_mut() = Some(b"AWS_PROFILE=recovered\0".to_vec());
        assert_credential_eq(
            &(get_env_api_key_with_platform(&p, &State::default(), "amazon-bedrock").unwrap()),
            &(Some("<authenticated>".into())),
            "bedrock_checks_direct_signals_before_proc_signals",
        );
        assert_eq!(*p.log.borrow(), direct_order[..=index]);
        assert_eq!(p.reads.get(), 0);
        assert_eq!(p.recovery_checks.get(), 0);
        let p = Controlled::default();
        p.bun.set(true);
        *p.bytes.borrow_mut() = Some(format!("{key}=recovered\0").into_bytes());
        assert_credential_eq(
            &(get_env_api_key_with_platform(&p, &State::default(), "amazon-bedrock").unwrap()),
            &(Some("<authenticated>".into())),
            "bedrock_checks_direct_signals_before_proc_signals",
        );
        assert_eq!(*p.log.borrow(), direct_order);
        assert_eq!(p.reads.get(), 1);
        assert_eq!(p.recovery_checks.get(), index + 1);
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
    let p = Controlled::default();
    p.bun.set(true);
    *p.bytes.borrow_mut() = Some(b"AWS_ACCESS_KEY_ID=id\0AWS_SECRET_ACCESS_KEY=secret\0".to_vec());
    assert!(
        get_env_api_key_with_platform(&p, &State::default(), "amazon-bedrock")
            .unwrap()
            .is_some()
    );
    assert_eq!(p.recovery_checks.get(), 3);
    for bytes in [
        b"AWS_ACCESS_KEY_ID=id\0".as_slice(),
        b"AWS_SECRET_ACCESS_KEY=secret\0",
        b"AWS_SESSION_TOKEN=token\0",
        b"AWS_PROFILE=\0AWS_BEARER_TOKEN_BEDROCK=\0",
        b"",
    ] {
        let p = Controlled::default();
        p.bun.set(true);
        *p.bytes.borrow_mut() = Some(bytes.to_vec());
        assert!(
            get_env_api_key_with_platform(&p, &State::default(), "amazon-bedrock")
                .unwrap()
                .is_none()
        );
        assert_eq!(p.reads.get(), 1);
    }
    for (direct, recovered) in [
        ("AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY"),
        ("AWS_SECRET_ACCESS_KEY", "AWS_ACCESS_KEY_ID"),
    ] {
        let p = Controlled::default();
        p.bun.set(true);
        p.put(direct, "direct-inherited-property");
        *p.bytes.borrow_mut() = Some(format!("{recovered}=recovered\0").into_bytes());
        assert!(
            get_env_api_key_with_platform(&p, &State::default(), "amazon-bedrock")
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn environment_helpers_keep_domain_names() {
    let source = include_str!("../../src/builtins/env_api_keys.rs");
    assert!(source.contains("fn has_vertex_adc_credentials("));
    assert!(source.contains("fn get_proc_env("));
}

#[test]
fn host_getter_thrown_array_keeps_payload() {
    let p = Controlled::default();
    p.failures.borrow_mut().insert(
        "OPENAI_API_KEY".into(),
        platform::HostThrown::Json(r#"["controlled-error"]"#.into()),
    );
    let failure = expect_host_failure(get_env_api_key_with_platform(
        &p,
        &State::default(),
        "openai",
    ));
    assert_eq!(
        crate::format_thrown_value(&failure).unwrap(),
        "controlled-error"
    );
    assert!(
        matches!(failure, ThrownValue::Json(value) if value == serde_json::json!(["controlled-error"]))
    );
}

#[test]
fn host_getter_thrown_object_keeps_payload() {
    let p = Controlled::default();
    p.failures.borrow_mut().insert(
        "OPENAI_API_KEY".into(),
        platform::HostThrown::Json(r#"{"message":"blocked","code":"EHOST"}"#.into()),
    );
    let failure = expect_host_failure(get_env_api_key_with_platform(
        &p,
        &State::default(),
        "openai",
    ));
    assert_eq!(
        crate::format_thrown_value(&failure).unwrap(),
        "[object Object]"
    );
    assert!(
        matches!(failure, ThrownValue::Json(value) if value == serde_json::json!({"message":"blocked","code":"EHOST"}))
    );
}

#[test]
fn host_getter_thrown_error_keeps_metadata() {
    let p = Controlled::default();
    p.failures.borrow_mut().insert(
        "OPENAI_API_KEY".into(),
        platform::HostThrown::Error(Box::new(crate::Error {
            name: "HostError".into(),
            message: "blocked".into(),
            stack: Some("controlled-stack".into()),
            code: Some(ThrownValue::Json("EHOST".into())),
            errno: Some(ThrownValue::Number(5.0)),
            cause: Some(ThrownValue::Json(serde_json::json!({"message":"root"}))),
        })),
    );
    let failure = expect_host_failure(get_env_api_key_with_platform(
        &p,
        &State::default(),
        "openai",
    ));
    let diagnostic = crate::extract_diagnostic_error(&failure).unwrap();
    assert_eq!(diagnostic.message, "blocked");
    assert_eq!(diagnostic.name.as_deref(), Some("HostError"));
    assert_eq!(diagnostic.stack.as_deref(), Some("controlled-stack"));
    assert_eq!(
        diagnostic.code,
        Some(crate::DiagnosticCode::String("EHOST".into()))
    );
    let ThrownValue::Error(error) = failure else {
        panic!("expected Error")
    };
    assert!(matches!(error.errno, Some(ThrownValue::Number(5.0))));
    assert!(
        matches!(error.cause, Some(ThrownValue::Json(value)) if value == serde_json::json!({"message":"root"}))
    );
}

#[test]
fn browser_boolean_profile_uses_truthiness() {
    let p = Controlled::default();
    p.host_values
        .borrow_mut()
        .insert("AWS_PROFILE".into(), (true, ThrownValue::Json(true.into())));
    assert_credential_eq(
        &(get_env_api_key_with_platform(&p, &State::default(), "amazon-bedrock")
            .unwrap()
            .as_deref()),
        &(Some("<authenticated>")),
        "browser_boolean_profile_uses_truthiness",
    );
}

#[test]
fn browser_numeric_key_is_discovered_and_coerced() {
    let p = Controlled::default();
    p.host_values
        .borrow_mut()
        .insert("OPENAI_API_KEY".into(), (true, ThrownValue::Number(1.0)));
    assert_eq!(
        find_env_keys_with_platform(&p, &State::default(), "openai").unwrap(),
        Some(vec!["OPENAI_API_KEY".into()])
    );
    assert_credential_eq(
        &(get_env_api_key_with_platform(&p, &State::default(), "openai")
            .unwrap()
            .as_deref()),
        &(Some("1")),
        "browser_numeric_key_is_discovered_and_coerced",
    );
}

#[test]
fn browser_falsy_values_remain_absent() {
    for value in [ThrownValue::Number(0.0), ThrownValue::Json("".into())] {
        let p = Controlled::default();
        p.host_values
            .borrow_mut()
            .insert("OPENAI_API_KEY".into(), (false, value.clone()));
        p.host_values
            .borrow_mut()
            .insert("AWS_PROFILE".into(), (false, value));
        assert_eq!(
            find_env_keys_with_platform(&p, &State::default(), "openai").unwrap(),
            None
        );
        assert_credential_eq(
            &(get_env_api_key_with_platform(&p, &State::default(), "openai").unwrap()),
            &(None),
            "browser_falsy_values_remain_absent",
        );
        assert_credential_eq(
            &(get_env_api_key_with_platform(&p, &State::default(), "amazon-bedrock").unwrap()),
            &(None),
            "browser_falsy_values_remain_absent",
        );
    }
}

#[test]
fn browser_discovery_and_ambient_checks_do_not_coerce_values() {
    let p = Controlled::default();
    let value = ThrownValue::StringCoercion(std::sync::Arc::new(|| {
        Err(ThrownValue::Json("coercion failed".into()))
    }));
    p.host_values
        .borrow_mut()
        .insert("OPENAI_API_KEY".into(), (true, value.clone()));
    p.host_values
        .borrow_mut()
        .insert("AWS_PROFILE".into(), (true, value));
    assert_eq!(
        find_env_keys_with_platform(&p, &State::default(), "openai").unwrap(),
        Some(vec!["OPENAI_API_KEY".into()])
    );
    assert_credential_eq(
        &(get_env_api_key_with_platform(&p, &State::default(), "amazon-bedrock")
            .unwrap()
            .as_deref()),
        &(Some("<authenticated>")),
        "browser_discovery_and_ambient_checks_do_not_coerce_values",
    );
    assert_credential_eq(
        &(crate::format_thrown_value(&expect_host_failure(get_env_api_key_with_platform(
            &p,
            &State::default(),
            "openai",
        )))
        .unwrap()),
        &("coercion failed"),
        "browser_discovery_and_ambient_checks_do_not_coerce_values",
    );
}
