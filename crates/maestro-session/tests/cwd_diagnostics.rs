//! Missing stored working-directory diagnosis and its exact messages.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::support;
use maestro_session::SessionManager;
use maestro_session::session_cwd::{
    MissingSessionCwdError, SessionCwdIssue, assert_session_cwd_exists,
    format_missing_session_cwd_error, format_missing_session_cwd_prompt,
    get_missing_session_cwd_issue,
};
use maestro_session::session_manager::ReadonlySessionManager;
use std::path::Path;
use support::scratch::Scratch;

/// Write a one-record history whose header holds the given working directory.
fn stored_history(scratch: &Scratch, cwd: &str) -> String {
    let path = Path::new(scratch.text()).join("stored.jsonl");
    let header = serde_json::json!({
        "type": "session", "version": 3, "id": "stored", "cwd": cwd,
        "timestamp": "2025-01-01T00:00:00.000Z"
    });
    std::fs::write(&path, format!("{header}\n")).unwrap();
    path.to_str().unwrap().to_owned()
}

#[test]
fn maestro_persisted_session_reports_missing_cwd() {
    let scratch = Scratch::new();
    let missing = Path::new(scratch.text()).join("gone");
    let file = stored_history(&scratch, missing.to_str().unwrap());
    let manager = SessionManager::open(&file, None, None).unwrap();
    assert_eq!(
        get_missing_session_cwd_issue(&manager, "/fallback"),
        Some(SessionCwdIssue {
            session_file: Some(file),
            session_cwd: missing.to_str().unwrap().to_owned(),
            fallback_cwd: "/fallback".to_owned(),
        })
    );
}

#[test]
fn maestro_open_cwd_override_preserves_header() {
    let scratch = Scratch::new();
    let missing = Path::new(scratch.text()).join("gone");
    let file = stored_history(&scratch, missing.to_str().unwrap());
    let before = std::fs::read(&file).unwrap();
    let manager = SessionManager::open(&file, None, Some(scratch.text())).unwrap();
    assert_eq!(manager.get_cwd(), scratch.text());
    assert_eq!(get_missing_session_cwd_issue(&manager, "/fallback"), None);
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn cwd_diagnosis_skips_memory_empty_and_existing_paths() {
    let scratch = Scratch::new();
    let missing = Path::new(scratch.text()).join("gone");
    let memory = SessionManager::in_memory(Some(missing.to_str().unwrap())).unwrap();
    assert_eq!(get_missing_session_cwd_issue(&memory, "/fallback"), None);
    let empty_cwd = stored_history(&scratch, "");
    let manager = SessionManager::open(&empty_cwd, None, Some("")).unwrap();
    assert_eq!(get_missing_session_cwd_issue(&manager, "/fallback"), None);
    let healthy = SessionManager::open(&empty_cwd, None, Some(scratch.text())).unwrap();
    assert_eq!(
        get_missing_session_cwd_issue(&healthy, "/does/not/exist"),
        None
    );
}

#[test]
fn cwd_diagnosis_accepts_existing_files_and_unpublished_sessions() {
    let scratch = Scratch::new();
    let dir = Path::new(scratch.text());
    let file = stored_history(&scratch, "/unused");
    let plain = dir.join("plain");
    std::fs::write(&plain, "x").unwrap();
    let manager = SessionManager::open(&file, None, plain.to_str()).unwrap();
    assert_eq!(get_missing_session_cwd_issue(&manager, "/fallback"), None);
    let unpublished = SessionManager::create(
        dir.join("absent").to_str().unwrap(),
        Some(scratch.text()),
        "unused",
    )
    .unwrap();
    assert!(unpublished.get_session_file().is_some());
    assert!(get_missing_session_cwd_issue(&unpublished, "/fallback").is_some());
}

#[cfg(unix)]
#[test]
fn cwd_diagnosis_follows_symlinks_to_existing_targets_only() {
    let scratch = Scratch::new();
    let dir = Path::new(scratch.text());
    let file = stored_history(&scratch, "/unused");
    let plain = dir.join("plain");
    std::fs::write(&plain, "x").unwrap();
    let live = dir.join("live");
    let dangling = dir.join("dangling");
    std::os::unix::fs::symlink(&plain, &live).unwrap();
    std::os::unix::fs::symlink(dir.join("nowhere"), &dangling).unwrap();
    for (cwd, diagnosed) in [(&live, false), (&dangling, true)] {
        let manager = SessionManager::open(&file, None, cwd.to_str()).unwrap();
        assert_eq!(
            get_missing_session_cwd_issue(&manager, "/fallback").is_some(),
            diagnosed,
            "{}",
            cwd.display()
        );
    }
}

#[test]
fn cwd_formatting_keeps_exact_messages_and_optional_file() {
    let issue = |file: Option<&str>| SessionCwdIssue {
        session_file: file.map(str::to_owned),
        session_cwd: "/gone\n/dir é".to_owned(),
        fallback_cwd: "/here\n".to_owned(),
    };
    let head = "Stored session working directory does not exist: /gone\n/dir é";
    let tail = "\nCurrent working directory: /here\n";
    assert_eq!(
        format_missing_session_cwd_error(&issue(Some("/s/a.jsonl"))),
        format!("{head}\nSession file: /s/a.jsonl{tail}")
    );
    for file in [None, Some("")] {
        assert_eq!(
            format_missing_session_cwd_error(&issue(file)),
            format!("{head}{tail}")
        );
    }
    assert_eq!(
        format_missing_session_cwd_prompt(&issue(Some("/s/a.jsonl"))),
        "cwd from session file does not exist\n/gone\n/dir é\n\ncontinue in current cwd\n/here\n"
    );
}

#[test]
fn cwd_assertion_returns_typed_error_with_original_issue() {
    let scratch = Scratch::new();
    let healthy = stored_history(&scratch, scratch.text());
    let manager = SessionManager::open(&healthy, None, None).unwrap();
    assert!(assert_session_cwd_exists(&manager, "/fallback").is_ok());
    let missing = Path::new(scratch.text()).join("gone");
    let file = stored_history(&scratch, missing.to_str().unwrap());
    let manager = SessionManager::open(&file, None, None).unwrap();
    let error = assert_session_cwd_exists(&manager, "/fallback").unwrap_err();
    assert_eq!(error.name(), "MissingSessionCwdError");
    assert_eq!(error.issue.session_file.as_deref(), Some(file.as_str()));
    assert_eq!(error.issue.session_cwd, missing.to_str().unwrap());
    assert_eq!(error.issue.fallback_cwd, "/fallback");
    assert_eq!(
        error.to_string(),
        format_missing_session_cwd_error(&error.issue)
    );
}

#[test]
fn cwd_error_message_is_captured_before_issue_edits() {
    let mut error = MissingSessionCwdError::new(SessionCwdIssue {
        session_file: None,
        session_cwd: "/gone".to_owned(),
        fallback_cwd: "/here".to_owned(),
    });
    let captured = error.to_string();
    error.issue.session_cwd = "/other".to_owned();
    assert_eq!(error.issue.session_cwd, "/other");
    assert_eq!(error.to_string(), captured);
}
