//! Deterministic whole-UUID collision witnesses.
#![cfg(test)]
use super::generate_id;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

/// Consumed private collision case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Unique corpus label.
    id: String,
    /// Owning witness.
    test: String,
    /// Private operation name.
    operation: String,
    /// Occupied prefixes.
    existing: Vec<String>,
    /// Complete candidate stream.
    candidates: Vec<String>,
    /// Returned identity and consumed count.
    expected: Expected,
}
/// Complete collision result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Returned short or full identity.
    id: String,
    /// Number of whole UUIDs consumed.
    calls: usize,
}
/// Consume a single collision case, checking the result and exhaustion.
fn collision(name: &str) {
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("../../tests/fixtures/session_history.json")).unwrap();
    let cases: Vec<Case> = rows
        .into_iter()
        .filter(|row| row["operation"] == "generate_id")
        .map(|row| serde_json::from_value(row).unwrap())
        .collect();
    assert_eq!(cases.len(), 4);
    let selected: Vec<_> = cases.into_iter().filter(|case| case.test == name).collect();
    assert_eq!(selected.len(), 1);
    let case = &selected[0];
    assert_eq!(case.operation, "generate_id");
    assert!(case.id.starts_with("id-"));
    let mut candidates = case
        .candidates
        .iter()
        .map(|candidate| Uuid::parse_str(candidate).unwrap());
    let mut calls = 0;
    let result = generate_id(
        |id| case.existing.iter().any(|existing| existing == id),
        || {
            calls += 1;
            candidates.next().unwrap()
        },
    );
    assert_eq!(
        result, case.expected.id,
        "case {} returned identity",
        case.id
    );
    assert_eq!(
        calls, case.expected.calls,
        "case {} candidate count",
        case.id
    );
    assert_eq!(
        candidates.next(),
        None,
        "case {} candidate exhaustion",
        case.id
    );
}

#[test]
fn entry_id_first_candidate_is_short() {
    collision("entry_id_first_candidate_is_short");
}
#[test]
fn entry_id_retries_existing_prefix() {
    collision("entry_id_retries_existing_prefix");
}
#[test]
fn entry_id_accepts_the_hundredth_candidate() {
    collision("entry_id_accepts_the_hundredth_candidate");
}
#[test]
fn entry_id_falls_back_after_one_hundred_collisions() {
    collision("entry_id_falls_back_after_one_hundred_collisions");
}

#[test]
fn conversion_ids_track_prior_allocations() {
    let corpus: Value =
        serde_json::from_str(include_str!("../../tests/fixtures/session_files.json")).unwrap();
    let case = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["test"] == "conversion_ids_track_prior_allocations")
        .unwrap();
    let mut entries = case["input"].as_array().unwrap().clone();
    let mut candidates = [
        Uuid::from_u128(0xaaaa_aaaa << 96),
        Uuid::from_u128(0xaaaa_aaaa << 96),
        Uuid::from_u128(0xbbbb_bbbb << 96),
    ]
    .into_iter();
    super::conversions::migrate(&mut entries, || candidates.next().unwrap()).unwrap();
    let context = super::build_session_context(
        &entries,
        maestro_request::presence::Presence::Missing,
        None::<&std::collections::HashMap<_, _>>,
    )
    .unwrap();
    assert_eq!(
        serde_json::json!({"entries":entries,"context":context}),
        case["expected"]["ok"]
    );
    assert!(candidates.next().is_none());
    assert_eq!(case["id"], "collision");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn failed_probe_releases_its_owned_file() {
    /// An owned native file whose failing read observes the original native error.
    struct Failing {
        /// Native descriptor owned by this reader.
        file: Option<std::fs::File>,
        /// Completion witness after descriptor release.
        released: std::rc::Rc<std::cell::Cell<bool>>,
    }
    impl std::io::Read for Failing {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from_raw_os_error(5))
        }
    }
    impl Drop for Failing {
        fn drop(&mut self) {
            drop(self.file.take());
            self.released.set(true);
        }
    }
    let released = std::rc::Rc::new(std::cell::Cell::new(false));
    let path = std::env::temp_dir().join(format!("maestro-reader-{}", Uuid::new_v4()));
    let reader = Failing {
        file: Some(std::fs::File::create(&path).unwrap()),
        released: released.clone(),
    };
    assert_eq!(
        super::files::probe(reader).unwrap_err().raw_os_error(),
        Some(5)
    );
    assert!(released.get());
    std::fs::remove_file(path).unwrap();
    #[cfg(unix)]
    assert!(super::files::probe(std::fs::File::open(std::env::temp_dir()).unwrap()).is_err());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn append_failure_keeps_memory_and_partial_bytes() {
    use std::io::Write;
    let directory = std::env::temp_dir().join(format!("maestro-partial-{}", Uuid::new_v4()));
    let mut manager =
        super::SessionManager::create("cwd", Some(directory.to_str().unwrap()), "").unwrap();
    let path = manager.get_session_file().unwrap().to_owned();
    std::fs::create_dir(&path).unwrap();
    assert!(
        manager
            .append_message(serde_json::json!({"role":"assistant","content":"accepted"}))
            .is_err()
    );
    std::fs::remove_dir(&path).unwrap();
    let error = manager
        .publish(|path, bytes| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?
                .write_all(&bytes[..7])?;
            Err(std::io::Error::from_raw_os_error(5))
        })
        .unwrap_err();
    assert_eq!(error.raw_os_error(), Some(5));
    assert_eq!(std::fs::read(&path).unwrap(), b"{\"type\"");
    assert_eq!(manager.get_entries().len(), 1);
    manager
        .append_message(serde_json::json!({"role":"user","content":"next"}))
        .unwrap();
    let bytes = std::fs::read_to_string(&path).unwrap();
    assert!(bytes.starts_with("{\"type\"{\"type\":\"session\""));
    assert_eq!(super::parse_session_entries(&bytes).len(), 2);
    std::fs::remove_dir_all(directory).unwrap();
}
