//! The standalone account file at a caller-chosen path: loading, saving every member in
//! order as two-space JSON, refusing a root that is not an object and reporting write
//! failures, compared with the recorded account cases.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::corpus;
use maestro_app::model_account::cli::{AccountFileError, load_auth, save_auth};
use serde_json::{Map, Value, json};

/// The recorded account case `name`.
fn expected(name: &str) -> Value {
    corpus("account")[name].clone()
}

/// Seeds `auth.json` with `seed` in a fresh directory, loads it, adds `alpha` and saves it;
/// returns the loaded members, the save outcome and the file afterwards.
fn run(
    seed: Option<&str>,
) -> (
    Map<String, Value>,
    Result<(), AccountFileError>,
    Option<String>,
) {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let path = format!("{}/auth.json", scratch.path().display());
    if let Some(seed) = seed {
        std::fs::write(&path, seed).unwrap();
    }
    let loaded = load_auth(&path);
    let mut auth = load_auth(&path);
    auth.insert("alpha".into(), json!({"type": "oauth"}));
    let saved = save_auth(&path, &auth);
    (loaded, saved, std::fs::read_to_string(&path).ok())
}

/// The recorded seed of the object case.
fn object_seed() -> String {
    serde_json::to_string_pretty(
        &json!({"beta": {"type": "oauth"}, "gamma": {"type": "api_key", "key": "k"}}),
    )
    .unwrap()
}

#[test]
fn loads_a_missing_account_file_as_an_empty_object() {
    let expected = expected("missing_file");
    let (loaded, saved, text) = run(None);
    assert_eq!(json!(loaded), expected["loaded"]["ok"]);
    assert!(saved.is_ok());
    assert_eq!(json!(text), expected["file_text"]);
}

#[test]
fn loads_an_unreadable_account_file_as_an_empty_object() {
    let expected = expected("malformed_file");
    let (loaded, saved, text) = run(Some("{ not json"));
    assert_eq!(json!(loaded), expected["loaded"]["ok"]);
    assert!(saved.is_ok());
    assert_eq!(json!(text), expected["file_text"]);
}

#[test]
fn keeps_unrelated_providers_when_saving() {
    let expected = expected("object_root");
    let (loaded, saved, _) = run(Some(&object_seed()));
    assert_eq!(json!(loaded), expected["loaded"]["ok"]);
    assert!(saved.is_ok());
}

#[test]
fn writes_two_space_json_without_a_trailing_newline() {
    let expected = expected("object_root");
    let (_, _, text) = run(Some(&object_seed()));
    let text = text.unwrap();
    assert_eq!(json!(text), expected["file_text"]);
    assert_eq!(json!(text.len()), expected["file_bytes"]);
}

#[test]
fn writes_integral_expiries_as_integers() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let path = format!("{}/auth.json", scratch.path().display());
    std::fs::write(&path, r#"{"gamma": {"type": "oauth"}}"#).unwrap();
    let mut auth = load_auth(&path);
    let expires = 1_700_000_000_000.0_f64;
    auth.insert(
        "alpha".into(),
        json!({"type": "oauth", "refresh": "r", "expires": expires}),
    );
    save_auth(&path, &auth).unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "{\n  \"gamma\": {\n    \"type\": \"oauth\"\n  },\n  \"alpha\": {\n    \"type\": \"oauth\",\n    \"refresh\": \"r\",\n    \"expires\": 1700000000000\n  }\n}"
    );
}

#[test]
fn refuses_a_non_object_root_without_overwriting_it() {
    for (name, seed) in [
        ("array_root", "[\n  {\n    \"type\": \"oauth\"\n  }\n]"),
        ("scalar_number_root", "5"),
        ("scalar_string_root", "\"text\""),
        ("null_root", "null"),
    ] {
        let expected = expected(name);
        let (loaded, saved, text) = run(Some(seed));
        assert!(loaded.is_empty(), "{name}");
        assert!(
            matches!(saved, Err(AccountFileError::NotAnObject)),
            "{name}"
        );
        assert_eq!(json!(text), json!(seed), "{name}");
        assert_eq!(json!(text), expected["file_text"], "{name}");
    }
}

#[cfg(unix)]
#[test]
fn propagates_write_failures() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path().display().to_string();
    let Some(_restore) = maestro_test_directory::deny(scratch.path(), 0o500).unwrap() else {
        return;
    };
    let auth = Map::from_iter([("alpha".to_owned(), json!({"type": "oauth"}))]);
    let saved = save_auth(&format!("{root}/auth.json"), &auth);
    assert!(
        matches!(&saved, Err(AccountFileError::Write(error)) if error.kind() == std::io::ErrorKind::PermissionDenied),
        "{saved:?}"
    );
}

#[test]
fn saving_reaches_a_hard_linked_account_file_in_place() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let path = format!("{}/auth.json", scratch.path().display());
    let linked = format!("{}/linked.json", scratch.path().display());
    std::fs::write(&path, object_seed()).unwrap();
    std::fs::hard_link(&path, &linked).unwrap();
    let mut auth = load_auth(&path);
    auth.insert("alpha".into(), json!({"type": "oauth"}));

    save_auth(&path, &auth).unwrap();

    assert!(std::fs::read_to_string(&path).unwrap().contains("alpha"));
    assert_eq!(
        std::fs::read_to_string(&linked).unwrap(),
        std::fs::read_to_string(&path).unwrap()
    );
}

/// Re-runs the calling test in a child with umask 022: true in the parent once the child
/// passed, false inside the child.
#[cfg(unix)]
fn relaunched_under_umask_022(test: &str) -> bool {
    use maestro_test_process::TestProcess;
    if std::env::var_os("ACCOUNT_FILE_UMASK_CHILD").is_some() {
        return false;
    }
    let output = TestProcess::current(test)
        .unwrap()
        .shell_setup("/bin/sh", "umask 022")
        .env("ACCOUNT_FILE_UMASK_CHILD", "1")
        .single_threaded()
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    true
}

#[cfg(unix)]
#[test]
fn saving_creates_the_account_file_private_to_its_owner() {
    use std::os::unix::fs::PermissionsExt;
    if relaunched_under_umask_022("saving_creates_the_account_file_private_to_its_owner") {
        return;
    }
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let path = format!("{}/auth.json", scratch.path().display());
    let auth = Map::from_iter([("alpha".to_owned(), json!({"type": "oauth"}))]);

    save_auth(&path, &auth).unwrap();

    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}
