//! Native project and all-project session listing through the public history owner.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

/// Recorded listing cases and disposable histories.
mod support {
    pub mod discovery;
    pub mod scratch;
}
use maestro_request::presence::Presence;
use maestro_session::{SessionInfo, SessionManager, parse_session_entries};
use serde_json::{Value, json};
use std::io;
use std::path::Path;
use std::sync::mpsc;
use support::discovery::replay;
use support::scratch::Scratch;

/// Compare recorded cases whose results do not depend on the timezone.
fn replay_fixed(names: &[&str]) {
    replay(names, |_, _| {});
}

#[test]
fn discovery_framing_requires_only_the_first_header_type() {
    replay_fixed(&[
        "empty",
        "malformed_only",
        "wrong_first_record",
        "null_first_record",
        "header_only",
        "outer_bom",
        "interior_bom",
        "nel_prefix",
    ]);
}

#[test]
fn discovery_keeps_open_metadata_and_cwd_fallback() {
    replay_fixed(&[
        "missing_identity",
        "null_identity",
        "open_identity",
        "sentinel_metadata",
        "missing_cwd",
        "null_cwd",
        "wrong_cwd",
        "empty_cwd",
        "parent_null",
        "parent_empty",
        "parent_open",
        "parent_text",
    ]);
}

#[test]
fn discovery_created_dates_preserve_invalid_observations() {
    replay_fixed(&[
        "missing_created",
        "invalid_created",
        "null_created",
        "numeric_created",
    ]);
}

#[test]
fn discovery_counts_messages_without_indexing_other_roles() {
    replay_fixed(&[
        "missing_content",
        "wrong_role",
        "missing_role",
        "null_role",
        "nonobject_record_after_header",
    ]);
}

#[test]
fn discovery_search_text_keeps_order_duplicates_and_spaces() {
    replay_fixed(&[
        "text_order_and_duplicates",
        "space_join",
        "text_missing_or_null",
    ]);
}

#[test]
fn discovery_first_message_uses_nonempty_user_text() {
    replay_fixed(&["first_user_ignores_empty", "assistant_only"]);
}

#[test]
fn discovery_names_scan_forward_and_keep_latest_clear() {
    replay_fixed(&[
        "name_clear",
        "name_null",
        "name_absent",
        "name_wrong",
        "name_wrong_then_valid",
    ]);
}

#[test]
fn discovery_name_whitespace_matches_framing() {
    replay_fixed(&["name_trim", "name_nel"]);
}

#[test]
fn discovery_reached_malformed_fields_discard_only_the_file() {
    replay_fixed(&[
        "null_message",
        "missing_message",
        "null_content",
        "number_content",
        "null_block",
        "null_record_after_header",
    ]);
    let scratch = Scratch::new();
    write_session(&scratch, "bad.jsonl", "bad", &[]);
    std::fs::write(
        Path::new(scratch.text()).join("bad.jsonl"),
        format!("{}\nnull\n", session_header("bad")),
    )
    .unwrap();
    write_session(&scratch, "good.jsonl", "good", &[("user", "hi", 5000)]);
    let listed = list(&scratch);
    assert_eq!(ids(&listed), ["good"]);
}

#[test]
fn discovery_ignores_unread_fields_and_uses_last_duplicate() {
    replay_fixed(&["unread_fields", "duplicate_selected_fields"]);
}

#[test]
fn maestro_session_recency_uses_conversation_activity() {
    replay_fixed(&[
        "activity_max_not_last",
        "tool_only_uses_header",
        "empty_text_still_activity",
    ]);
}

#[test]
fn discovery_numeric_timestamps_preempt_entry_fallback() {
    replay_fixed(&["activity_positive", "activity_zero", "activity_negative"]);
}

#[test]
fn discovery_missing_timestamp_uses_parsed_entry_time() {
    replay_fixed(&[
        "activity_absent",
        "activity_null",
        "activity_wrong",
        "activity_invalid_entry",
        "activity_date_only",
        "activity_offset",
        "missing_entry_time",
        "nonstring_entry_time",
    ]);
}

#[test]
fn discovery_local_dates_use_native_local_zone() {
    let name = "discovery_local_dates_use_native_local_zone";
    if std::env::var("MAESTRO_DATE_CHILD").as_deref() != Ok(name) {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture"])
            .env("TZ", "America/New_York")
            .env("MAESTRO_DATE_CHILD", name)
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    // Midnight local time in January is five hours after midnight UTC.
    replay(
        &["activity_local", "activity_date_only"],
        |case, expected| {
            let utc_midnight = 1_735_862_400_000_u64;
            let offset = if case == "activity_local" {
                18_000_000
            } else {
                0
            };
            expected["modified"] = json!(utc_midnight + offset);
        },
    );
}

#[test]
fn discovery_inactive_history_uses_header_then_mtime() {
    replay_fixed(&["bad_header_uses_mtime"]);
}

/// Header line of a stored session at a fixed instant.
fn session_header(id: &str) -> String {
    json!({"type": "session", "id": id, "cwd": "/project", "timestamp": "2025-01-02T03:04:05.000Z"})
        .to_string()
}

/// Write a session whose messages are `(role, text, timestamp)`.
fn session_text(id: &str, messages: &[(&str, &str, u64)]) -> String {
    let mut text = session_header(id);
    for (role, content, timestamp) in messages {
        let message = json!({"role": role, "content": content, "timestamp": timestamp});
        text.push('\n');
        text.push_str(&json!({"type": "message", "message": message}).to_string());
    }
    text
}

/// Write a session file below the scratch directory.
fn write_session(scratch: &Scratch, name: &str, id: &str, messages: &[(&str, &str, u64)]) {
    std::fs::write(
        Path::new(scratch.text()).join(name),
        session_text(id, messages),
    )
    .unwrap();
}

/// A current-thread runtime for driving the native listing.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
}

/// List a scratch directory without progress.
fn list(scratch: &Scratch) -> Vec<SessionInfo> {
    runtime()
        .block_on(SessionManager::list(
            "/unused",
            Some(scratch.text()),
            "/unused",
            None,
        ))
        .unwrap()
}

/// Identities of listed sessions, in result order.
fn ids(listed: &[SessionInfo]) -> Vec<&str> {
    listed
        .iter()
        .map(|info| match &info.id {
            Presence::Present(Value::String(id)) => id.as_str(),
            other => panic!("unexpected identity {other:?}"),
        })
        .collect()
}

/// File names in native enumeration order.
fn native_order(dir: &str) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect()
}

#[test]
fn directory_listing_filters_jsonl_and_keeps_stable_activity_ties() {
    let scratch = Scratch::new();
    let elsewhere = Scratch::new();
    let dir = Path::new(scratch.text());
    write_session(&scratch, "aa.jsonl", "aa", &[("user", "x", 3000)]);
    write_session(&scratch, "zz.jsonl", "zz", &[("user", "x", 3000)]);
    write_session(&scratch, "mid.jsonl", "mid", &[("user", "x", 2000)]);
    write_session(&scratch, "old.jsonl", "old", &[("user", "x", 1000)]);
    write_session(&scratch, "UPPER.JSONL", "upper", &[("user", "x", 9000)]);
    write_session(&scratch, "note.jsonl.bak", "bak", &[("user", "x", 9000)]);
    write_session(&elsewhere, "target.jsonl", "linked", &[("user", "x", 4000)]);
    std::os::unix::fs::symlink(
        Path::new(elsewhere.text()).join("target.jsonl"),
        dir.join("link.jsonl"),
    )
    .unwrap();
    std::fs::write(dir.join("bad.jsonl"), "{broken").unwrap();
    std::fs::create_dir(dir.join("folder.jsonl")).unwrap();
    let mut progress = Vec::new();
    let mut record = |loaded, total| {
        progress.push((loaded, total));
        Ok(())
    };
    let listed = runtime()
        .block_on(SessionManager::list(
            "/unused",
            Some(scratch.text()),
            "/unused",
            Some(&mut record),
        ))
        .unwrap();
    assert_eq!(
        progress,
        (1..=7).map(|loaded| (loaded, 7)).collect::<Vec<_>>()
    );
    let tied: Vec<_> = native_order(scratch.text())
        .into_iter()
        .filter(|name| name == "aa.jsonl" || name == "zz.jsonl")
        .map(|name| name.trim_end_matches(".jsonl").to_owned())
        .collect();
    let mut expected = vec!["linked"];
    expected.extend(tied.iter().map(String::as_str));
    expected.extend(["mid", "old"]);
    assert_eq!(ids(&listed), expected);
}

#[test]
fn project_listing_resolves_only_omitted_directory() {
    let scratch = Scratch::new();
    let agent = Path::new(scratch.text()).join("agent");
    let agent = agent.to_str().unwrap();
    let explicit = Scratch::new();
    write_session(&explicit, "one.jsonl", "one", &[("user", "x", 1000)]);
    let listed = runtime()
        .block_on(SessionManager::list(
            "/work/tree",
            Some(explicit.text()),
            agent,
            None,
        ))
        .unwrap();
    assert_eq!(ids(&listed), ["one"]);
    assert!(!Path::new(agent).exists());
    let listed = runtime()
        .block_on(SessionManager::list("/work/tree", None, agent, None))
        .unwrap();
    assert!(listed.is_empty());
    assert!(Path::new(agent).join("sessions/--work-tree--").is_dir());
    let empty = Scratch::new();
    assert!(list(&empty).is_empty());
}

#[test]
fn project_listing_default_creation_error_escapes() {
    let scratch = Scratch::new();
    let blocker = Path::new(scratch.text()).join("blocker");
    std::fs::write(&blocker, "file").unwrap();
    let error = runtime()
        .block_on(SessionManager::list(
            "/work",
            None,
            blocker.to_str().unwrap(),
            None,
        ))
        .unwrap_err();
    assert_ne!(error.kind(), io::ErrorKind::Other);
    let listed = runtime()
        .block_on(SessionManager::list(
            "/work",
            Some(blocker.to_str().unwrap()),
            "unused",
            None,
        ))
        .unwrap();
    assert!(listed.is_empty());
}

#[test]
fn project_listing_directory_failures_return_empty() {
    let scratch = Scratch::new();
    let missing = Path::new(scratch.text()).join("missing");
    let file = Path::new(scratch.text()).join("plain");
    std::fs::write(&file, "x").unwrap();
    let unreadable = Path::new(scratch.text()).join("locked");
    std::fs::create_dir(&unreadable).unwrap();
    std::fs::set_permissions(
        &unreadable,
        std::os::unix::fs::PermissionsExt::from_mode(0o000),
    )
    .unwrap();
    let empty = Scratch::new();
    let mut calls = 0;
    for dir in [
        missing.to_str().unwrap(),
        file.to_str().unwrap(),
        unreadable.to_str().unwrap(),
        "",
        empty.text(),
    ] {
        let mut count = |_, _| {
            calls += 1;
            Ok(())
        };
        let listed = runtime()
            .block_on(SessionManager::list(
                "/w",
                Some(dir),
                "unused",
                Some(&mut count),
            ))
            .unwrap();
        assert!(listed.is_empty());
    }
    assert_eq!(calls, 0);
    std::fs::set_permissions(
        &unreadable,
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .unwrap();
}

/// Create `<agent>/sessions/<project>/<file>` histories and return the agent directory.
fn projects(scratch: &Scratch, layout: &[(&str, &str, u64)]) -> String {
    for (project, id, time) in layout {
        let dir = Path::new(scratch.text()).join("sessions").join(project);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{id}.jsonl")),
            session_text(id, &[("user", "x", *time)]),
        )
        .unwrap();
    }
    scratch.text().to_owned()
}

#[test]
fn all_projects_lists_real_directories_and_ignores_root_files() {
    let scratch = Scratch::new();
    let agent = projects(
        &scratch,
        &[("a", "a1", 1000), ("b", "b1", 3000), ("b", "b2", 2000)],
    );
    let sessions = Path::new(&agent).join("sessions");
    std::fs::write(
        sessions.join("root.jsonl"),
        session_text("root", &[("user", "x", 9000)]),
    )
    .unwrap();
    let outside = Scratch::new();
    projects(&outside, &[("c", "linked", 8000)]);
    std::os::unix::fs::symlink(
        Path::new(outside.text()).join("sessions/c"),
        sessions.join("link"),
    )
    .unwrap();
    let listed = runtime().block_on(SessionManager::list_all(&agent, None));
    assert_eq!(ids(&listed), ["b1", "b2", "a1"]);
}

#[test]
fn all_projects_counts_candidates_before_reading() {
    let scratch = Scratch::new();
    let agent = projects(&scratch, &[("a", "a1", 1000), ("b", "b1", 2000)]);
    let late = Path::new(&agent).join("sessions/b/late.jsonl");
    let mut seen = Vec::new();
    let mut record = |loaded, total| {
        seen.push((loaded, total));
        std::fs::write(&late, session_text("late", &[("user", "x", 5000)])).unwrap();
        Ok(())
    };
    let listed = runtime().block_on(SessionManager::list_all(&agent, Some(&mut record)));
    assert_eq!(seen, [(1, 2), (2, 2)]);
    assert_eq!(listed.len(), 2);
}

#[test]
fn all_projects_skips_unreadable_children() {
    let scratch = Scratch::new();
    let agent = projects(&scratch, &[("good", "g1", 1000), ("locked", "l1", 2000)]);
    let sessions = Path::new(&agent).join("sessions");
    std::fs::write(sessions.join("good/bad.jsonl"), "{broken").unwrap();
    let locked = sessions.join("locked");
    std::fs::set_permissions(&locked, std::os::unix::fs::PermissionsExt::from_mode(0o000)).unwrap();
    let mut seen = Vec::new();
    let mut record = |loaded, total| {
        seen.push((loaded, total));
        Ok(())
    };
    let listed = runtime().block_on(SessionManager::list_all(&agent, Some(&mut record)));
    std::fs::set_permissions(&locked, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    let readable_anyway = std::fs::read_dir(&locked).is_ok() && ids(&listed).contains(&"l1");
    if !readable_anyway {
        assert_eq!(ids(&listed), ["g1"]);
        assert_eq!(seen, [(1, 2), (2, 2)]);
    }
}

#[test]
fn all_projects_top_level_failure_returns_empty() {
    let scratch = Scratch::new();
    let mut calls = 0;
    let mut count = |_, _| {
        calls += 1;
        Ok(())
    };
    let missing = runtime().block_on(SessionManager::list_all(scratch.text(), Some(&mut count)));
    assert!(missing.is_empty());
    assert!(!Path::new(scratch.text()).join("sessions").exists());
    std::fs::write(Path::new(scratch.text()).join("sessions"), "file").unwrap();
    let not_directory =
        runtime().block_on(SessionManager::list_all(scratch.text(), Some(&mut count)));
    assert!(not_directory.is_empty());
    assert_eq!(calls, 0);
}

/// A named pipe candidate: its reader blocks until a producer opens the write end.
fn fifo(dir: &Path, name: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    path
}

/// Open, fill and close a pipe on another thread, returning once the reader opened it.
fn produce(path: std::path::PathBuf, text: String) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || std::fs::write(path, text).unwrap())
}

#[test]
fn progress_observes_completion_order_before_stable_results() {
    for all_projects in [false, true] {
        let scratch = Scratch::new();
        let dir = if all_projects {
            Path::new(scratch.text()).join("sessions/p")
        } else {
            Path::new(scratch.text()).to_path_buf()
        };
        std::fs::create_dir_all(&dir).unwrap();
        let pipe = fifo(&dir, "pipe.jsonl");
        std::fs::write(dir.join("ready.jsonl"), session_text("ready", &[])).unwrap();
        let mut producers = Vec::new();
        let mut seen = Vec::new();
        let mut record = |loaded, total| {
            seen.push((loaded, total));
            if loaded == 1 {
                producers.push(produce(pipe.clone(), session_text("pipe", &[])));
            }
            Ok(())
        };
        let listed = if all_projects {
            runtime().block_on(SessionManager::list_all(scratch.text(), Some(&mut record)))
        } else {
            runtime()
                .block_on(SessionManager::list(
                    "/w",
                    Some(scratch.text()),
                    "unused",
                    Some(&mut record),
                ))
                .unwrap()
        };
        for producer in producers {
            producer.join().unwrap();
        }
        assert_eq!(seen, [(1, 2), (2, 2)]);
        let order: Vec<_> = native_order(dir.to_str().unwrap())
            .into_iter()
            .map(|name| name.trim_end_matches(".jsonl").to_owned())
            .collect();
        assert_eq!(ids(&listed), order);
    }
}

#[test]
fn invalid_candidates_still_advance_progress() {
    let scratch = Scratch::new();
    write_session(&scratch, "ok.jsonl", "ok", &[]);
    std::fs::write(Path::new(scratch.text()).join("junk.jsonl"), "nope").unwrap();
    std::fs::write(Path::new(scratch.text()).join("empty.jsonl"), "").unwrap();
    let mut seen = Vec::new();
    let mut record = |loaded, total| {
        seen.push((loaded, total));
        Ok(())
    };
    let listed = runtime()
        .block_on(SessionManager::list(
            "/w",
            Some(scratch.text()),
            "unused",
            Some(&mut record),
        ))
        .unwrap();
    assert_eq!(seen, [(1, 3), (2, 3), (3, 3)]);
    assert_eq!(listed.len(), 1);
}

#[test]
fn progress_error_returns_empty_without_draining_reads() {
    for all_projects in [false, true] {
        let scratch = Scratch::new();
        let dir = if all_projects {
            Path::new(scratch.text()).join("sessions/p")
        } else {
            Path::new(scratch.text()).to_path_buf()
        };
        std::fs::create_dir_all(&dir).unwrap();
        let pipe = fifo(&dir, "pipe.jsonl");
        std::fs::write(dir.join("ready.jsonl"), session_text("ready", &[])).unwrap();
        let mut calls = 0;
        let mut fail = |_, _| {
            calls += 1;
            Err(io::Error::other("stop"))
        };
        let runtime = runtime();
        let listed = if all_projects {
            runtime.block_on(SessionManager::list_all(scratch.text(), Some(&mut fail)))
        } else {
            runtime
                .block_on(SessionManager::list(
                    "/w",
                    Some(scratch.text()),
                    "unused",
                    Some(&mut fail),
                ))
                .unwrap()
        };
        assert!(listed.is_empty());
        assert_eq!(calls, 1);
        // The abandoned read is still blocked in the pipe open; release it before the runtime drops.
        produce(pipe, session_text("pipe", &[])).join().unwrap();
        drop(runtime);
    }
}

#[test]
fn discovery_reads_lossy_utf8_and_skips_failed_metadata() {
    let scratch = Scratch::new();
    let dir = Path::new(scratch.text());
    let mut bytes = session_text("lossy", &[("user", "before", 2000)]).into_bytes();
    bytes.extend_from_slice(b"\n{\"type\":\"message\",\"message\":{\"role\":\"user\",\"content\":\"bad \xff byte\",\"timestamp\":1}}\n");
    std::fs::write(dir.join("lossy.jsonl"), bytes).unwrap();
    let pipe = fifo(dir, "vanishing.jsonl");
    let (opened, wait) = mpsc::channel();
    let producer = {
        std::thread::spawn(move || {
            let mut writer = std::fs::OpenOptions::new().write(true).open(&pipe).unwrap();
            std::fs::remove_file(&pipe).unwrap();
            io::Write::write_all(&mut writer, session_text("gone", &[]).as_bytes()).unwrap();
            drop(writer);
            opened.send(()).unwrap();
        })
    };
    let listed = list(&scratch);
    wait.recv().unwrap();
    producer.join().unwrap();
    assert_eq!(ids(&listed), ["lossy"]);
    assert_eq!(listed[0].all_messages_text, "before bad \u{fffd} byte");
}

/// JSONL blocks of the session-format guide, where each fenced block starts a fresh parse.
fn guide_records() -> Vec<Value> {
    let guide = include_str!("../../../docs/sessions/session_format.md");
    guide
        .split("```jsonl\n")
        .skip(1)
        .flat_map(|block| parse_session_entries(block.split("```").next().unwrap()))
        .collect()
}

#[test]
fn session_format_examples_replay_all_record_kinds() {
    let records = guide_records();
    let kinds: Vec<_> = records
        .iter()
        .map(|record| record["type"].as_str().unwrap())
        .collect();
    for kind in [
        "session",
        "message",
        "model_change",
        "thinking_level_change",
        "compaction",
        "branch_summary",
        "custom",
        "custom_message",
        "label",
        "session_info",
    ] {
        assert!(kinds.contains(&kind), "{kind}");
    }
    assert_eq!(records[0]["version"], 3);
    assert!(records[0].get("parentId").is_none());
}
