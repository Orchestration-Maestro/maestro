//! Deterministic whole-UUID collision witnesses.
#![cfg(test)]
use super::{ReadonlySessionManager, generate_id};
#[cfg(not(target_arch = "wasm32"))]
use maestro_test_directory::{Scratch, temporary_root};
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
    let path = temporary_root().join(format!("maestro-reader-{}", Uuid::new_v4()));
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
    assert!(super::files::probe(std::fs::File::open(temporary_root()).unwrap()).is_err());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn append_failure_keeps_memory_and_partial_bytes() {
    use std::io::Write;
    let scratch = Scratch::new().unwrap();
    let directory = scratch.path().join("sessions");
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
}

#[test]
fn new_session_clears_resolved_labels_and_timestamps() {
    let mut manager = super::SessionManager::in_memory(Some("cwd")).unwrap();
    let old = manager
        .append_message(serde_json::json!({"role": "user", "content": "old", "timestamp": 1}))
        .unwrap();
    manager
        .append_label_change(&old, maestro_request::presence::Presence::Present("mark"))
        .unwrap();
    manager.new_session(super::NewSessionOptions::default());
    assert_eq!(manager.get_label(&old), None);
    // An independent session may legitimately reuse an identity; stage it directly.
    manager
        .by_id
        .insert(old.clone(), manager.file_entries.len());
    manager.file_entries.push(serde_json::json!({
        "type": "custom", "customType": "reuse", "id": old,
        "parentId": null, "timestamp": "2025-01-01T00:00:00.000Z",
    }));
    let tree = manager.get_tree();
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].entry["id"], old);
    assert_eq!(tree[0].label, None);
    assert_eq!(tree[0].label_timestamp, None);
}

#[test]
fn extraction_generates_unique_chained_label_ids() {
    let mut manager = super::SessionManager::in_memory(Some("cwd")).unwrap();
    let message = |id: &str, parent: Value| {
        serde_json::json!({"type": "message", "id": id, "parentId": parent, "timestamp": "t",
            "message": {"role": "user", "content": id, "timestamp": 1}})
    };
    manager.file_entries.push(message("aaaaaaaa", Value::Null));
    manager
        .file_entries
        .push(message("dddddddd", "aaaaaaaa".into()));
    for (position, id) in ["aaaaaaaa", "dddddddd"].into_iter().enumerate() {
        manager.by_id.insert(id.to_owned(), position + 1);
        manager.labels.insert(id.to_owned(), id.into());
        manager.label_timestamps.insert(id.to_owned(), "t".into());
    }
    let mut candidates = ["aaaaaaaa", "bbbbbbbb", "bbbbbbbb", "cccccccc"]
        .into_iter()
        .map(|prefix| Uuid::from_u128(u128::from_str_radix(prefix, 16).unwrap() << 96));
    manager
        .branch_history("dddddddd", || candidates.next().unwrap())
        .unwrap();
    assert_eq!(candidates.next(), None);
    let labels: Vec<_> = manager
        .get_entries()
        .into_iter()
        .filter(|entry| entry["type"] == "label")
        .collect();
    let chain: Vec<_> = labels
        .iter()
        .map(|entry| {
            (
                entry["id"].as_str(),
                entry["parentId"].as_str(),
                entry["targetId"].as_str(),
            )
        })
        .collect();
    assert_eq!(
        chain,
        [
            (Some("bbbbbbbb"), Some("dddddddd"), Some("aaaaaaaa")),
            (Some("cccccccc"), Some("bbbbbbbb"), Some("dddddddd")),
        ]
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn failed_extraction_keeps_a_consistent_owner() {
    let scratch = Scratch::new().unwrap();
    let directory = scratch.path();
    let mut manager =
        super::SessionManager::create("cwd", Some(directory.to_str().unwrap()), "").unwrap();
    manager
        .append_message(serde_json::json!({"role": "assistant", "content": "a", "timestamp": 1}))
        .unwrap();
    let previous = manager.get_session_file().unwrap().to_owned();
    let leaf = manager.get_leaf_id().unwrap().to_owned();
    manager.branch_history(&leaf, Uuid::new_v4).unwrap();
    let error = manager
        .publish_branched(|path, _| {
            std::fs::write(path, b"{\"type\"")?;
            Err(std::io::Error::from_raw_os_error(5))
        })
        .unwrap_err();
    assert_eq!(error.raw_os_error(), Some(5));
    let path = manager.get_session_file().unwrap().to_owned();
    assert_ne!(path, previous);
    assert!(manager.flushed);
    manager
        .append_message(serde_json::json!({"role": "user", "content": "next", "timestamp": 2}))
        .unwrap();
    let bytes = std::fs::read_to_string(&path).unwrap();
    assert!(bytes.starts_with("{\"type\"{\"type\":\"message\""));
    assert_eq!(bytes.lines().count(), 1);
    assert!(!bytes.contains("\"type\":\"session\""));
    std::fs::remove_dir_all(directory).unwrap();
    let error = manager.create_branched_session(&leaf).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert_ne!(manager.get_session_file().unwrap(), path);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn branch_publication_overwrites_a_competing_file() {
    let scratch = Scratch::new().unwrap();
    let mut manager =
        super::SessionManager::create("cwd", Some(scratch.path().to_str().unwrap()), "").unwrap();
    let leaf = manager
        .append_message(serde_json::json!({
            "role": "assistant", "content": "retained", "timestamp": 1
        }))
        .unwrap();
    BEFORE_BRANCH.set(Some(|path| {
        std::fs::write(path, b"competing bytes").unwrap();
    }));
    let path = manager.create_branched_session(&leaf).unwrap().unwrap();
    let records = super::parse_session_entries(&std::fs::read_to_string(&path).unwrap());
    assert_eq!(records.len(), 2);
    assert_eq!(records[1]["message"]["content"], "retained");
    assert_eq!(manager.get_session_file(), Some(path.as_str()));
}

thread_local! {
    /// One-shot concurrent publication at the branch writer boundary.
    pub(super) static BEFORE_BRANCH: std::cell::RefCell<Option<fn(&str)>> =
        std::cell::RefCell::new(None);
}

/// A readable source below a scratch root, forked into chosen directories.
#[cfg(not(target_arch = "wasm32"))]
struct ForkRig {
    /// Scratch root, removed on drop.
    root: Scratch,
    /// Source file with a header and two records.
    source: String,
    /// The two record lines of the source.
    records: [&'static str; 2],
}

#[cfg(not(target_arch = "wasm32"))]
impl ForkRig {
    /// Header line of every source.
    const HEADER: &'static str =
        r#"{"type":"session","id":"s","version":3,"cwd":"/src","timestamp":"t"}"#;

    fn new() -> Self {
        let root = Scratch::new().unwrap();
        let records = [
            r#"{"type":"custom","id":"a","parentId":null,"customType":"k"}"#,
            r#"{"type":"custom","id":"b","parentId":"a","customType":"k"}"#,
        ];
        let source = root.path().join("source.jsonl");
        let text = format!("{}\n{}\n{}\n", Self::HEADER, records[0], records[1]);
        std::fs::write(&source, text).unwrap();
        Self {
            root,
            source: source.to_str().unwrap().to_owned(),
            records,
        }
    }

    fn path(&self, name: &str) -> String {
        self.root.path().join(name).to_str().unwrap().to_owned()
    }

    fn fork(
        &self,
        dir: &str,
        write: impl FnMut(&str, &[u8]) -> std::io::Result<()>,
    ) -> std::io::Result<super::SessionManager> {
        super::SessionManager::fork_with(&self.source, "/t", Some(dir), "", write)
    }

    /// The single session history file created in the supplied directory.
    fn only_file(dir: &str) -> String {
        let files: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(Result::unwrap)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".jsonl"))
            .collect();
        assert_eq!(files.len(), 1);
        std::fs::read_to_string(files[0].path()).unwrap()
    }
}

/// Append the first seven bytes of a record, then fail like a full disk.
#[cfg(not(target_arch = "wasm32"))]
fn partial_append(path: &str, bytes: &[u8]) -> std::io::Result<()> {
    super::persistence::append(path, &bytes[..7])?;
    Err(std::io::Error::from_raw_os_error(5))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn fork_preserves_write_and_returned_owner_failure_order() {
    let rig = ForkRig::new();
    let real = super::persistence::append;

    let unreadable = rig.path("directory.jsonl");
    std::fs::create_dir(&unreadable).unwrap();
    let missing = rig.path("never");
    let error = super::SessionManager::fork_with(&unreadable, "/t", Some(&missing), "", real)
        .err()
        .unwrap();
    assert_ne!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(!std::fs::exists(&missing).unwrap());

    let file_target = rig.path("file-target");
    std::fs::write(&file_target, "keep").unwrap();
    assert!(rig.fork(&file_target, real).is_err());
    let child = std::path::Path::new(&file_target).join("child");
    assert!(rig.fork(child.to_str().unwrap(), real).is_err());
    assert_eq!(std::fs::read_to_string(&file_target).unwrap(), "keep");

    let first = rig.path("first");
    let error = rig.fork(&first, partial_append).err().unwrap();
    assert_eq!(error.raw_os_error(), Some(5));
    assert_eq!(ForkRig::only_file(&first), "{\"type\"");

    let later = rig.path("later");
    let mut calls = 0;
    let third = |path: &str, bytes: &[u8]| {
        calls += 1;
        if calls < 3 {
            return real(path, bytes);
        }
        partial_append(path, bytes)
    };
    assert!(rig.fork(&later, third).is_err());
    let written = ForkRig::only_file(&later);
    assert!(written.ends_with(&format!("{}\n{{\"type\"", rig.records[0])));
    assert_eq!(super::parse_session_entries(&written).len(), 2);

    let reopen = rig.path("reopen");
    let (mut calls, mut snapshot) = (0, Vec::new());
    let last = |path: &str, bytes: &[u8]| {
        calls += 1;
        real(path, bytes)?;
        if calls == 3 {
            snapshot = std::fs::read(path)?;
            std::fs::remove_file(path)?;
            std::fs::create_dir(path)?;
        }
        Ok(())
    };
    assert!(rig.fork(&reopen, last).is_err());
    assert_eq!(calls, 3);
    let snapshot = String::from_utf8(snapshot).unwrap();
    assert_eq!(super::parse_session_entries(&snapshot).len(), 3);
    invalid_later_id_fails_after_all_writes(&rig);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn fork_skips_a_null_record_and_writes_later_records() {
    let rig = ForkRig::new();
    let text = format!(
        "{}\n{}\nnull\n{}\n",
        ForkRig::HEADER,
        rig.records[0],
        rig.records[1]
    );
    std::fs::write(&rig.source, text).unwrap();
    let dir = rig.path("null-target");
    let forked = rig.fork(&dir, super::persistence::append).unwrap();
    let written = ForkRig::only_file(&dir);
    assert_eq!(written.lines().count(), 3);
    assert_eq!(forked.get_entries().len(), 2);
    assert_eq!(forked.get_entries()[1]["id"], "b");
    assert!(written.ends_with(&format!("{}\n", rig.records[1])));
}

/// A later numeric identity fails the returned owner's index after every write.
#[cfg(not(target_arch = "wasm32"))]
fn invalid_later_id_fails_after_all_writes(rig: &ForkRig) {
    let numeric = rig.path("numeric.jsonl");
    let text = format!(
        "{}\n{}\n{{\"type\":\"custom\",\"id\":5}}\n",
        ForkRig::HEADER,
        rig.records[0]
    );
    std::fs::write(&numeric, text).unwrap();
    let invalid = rig.path("invalid");
    let error = super::SessionManager::fork_from(&numeric, "/t", Some(&invalid), "")
        .err()
        .unwrap();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    let written = ForkRig::only_file(&invalid);
    assert_eq!(super::parse_session_entries(&written).len(), 3);
}

/// A reader over `head` then `body` repeated `count` times that counts the bytes it hands out.
#[cfg(not(target_arch = "wasm32"))]
struct Generated {
    /// Bytes produced first.
    head: Vec<u8>,
    /// Bytes produced after the head, `count` times.
    body: Vec<u8>,
    /// Repetitions of `body`.
    count: usize,
    /// Bytes produced so far.
    position: usize,
    /// The same total, observable while a consumer still holds the reader.
    handed: std::rc::Rc<std::cell::Cell<usize>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Generated {
    /// A header line, `middle` after it, and 4000 message lines of about two kilobytes.
    fn session(middle: &str) -> (Self, usize) {
        let head = format!("{{\"type\":\"session\",\"id\":\"s\"}}\n{middle}").into_bytes();
        let content = "x".repeat(2000);
        let body = format!(
            "{{\"type\":\"message\",\"message\":{{\"role\":\"user\",\"content\":\"{content}\",\"timestamp\":1}}}}\n"
        )
        .into_bytes();
        let line = body.len();
        let reader = Self {
            head,
            body,
            count: 4000,
            position: 0,
            handed: std::rc::Rc::default(),
        };
        (reader, line)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl std::io::Read for Generated {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let total = self.head.len() + self.body.len() * self.count;
        let count = buffer.len().min(total - self.position);
        for slot in &mut buffer[..count] {
            let at = self.position;
            *slot = if at < self.head.len() {
                self.head[at]
            } else {
                self.body[(at - self.head.len()) % self.body.len()]
            };
            self.position += 1;
        }
        self.handed.set(self.handed.get() + count);
        Ok(count)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn records_are_handed_over_while_the_file_is_still_being_read() {
    let (reader, line) = Generated::session("");
    let head = reader.head.len();
    let handed = std::rc::Rc::clone(&reader.handed);
    let (mut index, mut worst_lag) = (0, 0);
    super::records::for_each_entry(
        std::io::BufReader::new(reader),
        super::records::Admission::Nonempty,
        |_| {
            worst_lag = worst_lag.max(handed.get() - (head + index * line));
            index += 1;
            std::ops::ControlFlow::Continue(())
        },
    )
    .unwrap();
    assert_eq!(index, 4001);
    assert!(worst_lag <= 16 * 1024, "read {worst_lag} bytes ahead");
}

#[test]
fn parse_session_entries_drops_outer_ecmascript_whitespace_of_the_first_and_last_record() {
    let text = "\u{a0}\u{feff}{\"id\":\"a\"}\n\n{\"id\":\"b\"}\r\n{\"id\":\"c\"}\u{2028}";
    let ids: Vec<_> = super::parse_session_entries(text)
        .iter()
        .map(|entry| entry["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids, ["a", "b", "c"]);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn records_split_across_reader_chunks_keep_their_multibyte_text() {
    let text = "{\"id\":\"é€\"}\n{\"id\":\"b\"}";
    let reader = std::io::BufReader::with_capacity(1, text.as_bytes());
    let mut ids = Vec::new();
    super::records::for_each_entry(reader, super::records::Admission::Nonempty, |entry| {
        ids.push(entry["id"].as_str().unwrap().to_owned());
        std::ops::ControlFlow::Continue(())
    })
    .unwrap();
    assert_eq!(ids, ["é€", "b"]);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn listing_stops_reading_at_a_malformed_record() {
    let (reader, _) = Generated::session("{\"type\":\"message\",\"message\":null}\n");
    let handed = std::rc::Rc::clone(&reader.handed);
    assert!(super::discovery::scan_reader(std::io::BufReader::new(reader)).is_none());
    assert!(handed.get() <= 64 * 1024, "read {} bytes", handed.get());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn listing_keeps_at_most_ten_metadata_loads_in_flight() {
    let files: Vec<String> = (0..50).map(|n| format!("{n}.jsonl")).collect();
    let (running, peak) = (
        std::rc::Rc::new(std::cell::Cell::new(0_usize)),
        std::rc::Rc::new(std::cell::Cell::new(0_usize)),
    );
    let load = |_: String| {
        let (running, peak) = (std::rc::Rc::clone(&running), std::rc::Rc::clone(&peak));
        async move {
            running.set(running.get() + 1);
            peak.set(peak.get().max(running.get()));
            tokio::task::yield_now().await;
            running.set(running.get() - 1);
            None
        }
    };
    let mut loaded = 0;
    let mut progress = |done, _| {
        loaded = done;
        Ok(())
    };
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(super::discovery::collect(files, load, Some(&mut progress)));
    assert_eq!(loaded, 50);
    assert_eq!(peak.get(), 10);
}
