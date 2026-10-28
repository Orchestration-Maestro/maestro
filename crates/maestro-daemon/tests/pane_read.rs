//! Pane reads through the daemon's handler: the shared snapshot clipping, sources, formats and
//! the record a read answers with.
#![cfg(target_os = "linux")]
use crate::support::api::{
    TestApp, answer, daemon_with_workspaces, failure, request, run_in_pane, with_live_daemon,
};
use maestro_daemon::pane::terminal::TerminalReadSnapshot;
use maestro_protocol::schema::{
    ErrorBody, PaneInfo, PaneReadResult, ReadFormat, ReadSource, ResponseResult,
};
use serde_json::{Value, json};

/// The record a read answers with.
fn read_of(result: Result<ResponseResult, ErrorBody>) -> PaneReadResult {
    match result {
        Ok(ResponseResult::PaneRead { read }) => read,
        other => panic!("expected a read: {other:?}"),
    }
}

/// A live workspace whose shell has run `script` up to a line that starts with `marker`.
///
/// Returns the workspace identity and its root pane.
async fn printed(app: &mut TestApp, script: &str, marker: &str) -> (String, PaneInfo) {
    let created = answer(app, json!({"type": "workspace.create"})).await;
    let Ok(ResponseResult::WorkspaceCreated {
        workspace,
        root_pane,
        ..
    }) = created
    else {
        panic!("expected a created workspace: {created:?}");
    };
    let pane = crate::support::api::root_pane(app, 0);
    let terminal = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    run_in_pane(app, &terminal, &format!("{script}\n"), marker).await;
    (workspace.workspace_id, root_pane)
}

/// A read request for `pane`.
fn read(pane: &PaneInfo, source: &str, lines: Option<u32>, format: &str) -> Value {
    let mut wire = json!({
        "type": "pane.read", "pane_id": pane.pane_id, "source": source, "format": format
    });
    if let Some(lines) = lines {
        wire["lines"] = json!(lines);
    }
    wire
}

#[test]
fn snapshot_limits_keep_endings_and_report_truncation() {
    let snapshot = TerminalReadSnapshot::limited("one\ntwø\n三\n".into(), Some(2));
    assert_eq!(snapshot.text, "twø\n三\n");
    assert!(snapshot.truncated);
    let snapshot = TerminalReadSnapshot::limited("one\ntwo\nthree".into(), Some(1));
    assert_eq!(snapshot.text, "three");
    assert!(snapshot.truncated);
    let snapshot = TerminalReadSnapshot::limited("one\ntwo".into(), Some(0));
    assert_eq!(snapshot.text, "");
    assert!(snapshot.truncated);
    let snapshot = TerminalReadSnapshot::limited(String::new(), Some(2));
    assert_eq!(snapshot.text, "");
    assert!(!snapshot.truncated);
}

#[test]
fn an_absent_snapshot_limit_keeps_all_text() {
    let snapshot = TerminalReadSnapshot::limited("one\ntwo\n".into(), None);
    assert_eq!(snapshot.text, "one\ntwo\n");
    assert!(!snapshot.truncated);
}

#[test]
fn a_read_names_its_pane_workspace_and_tab_and_echoes_the_request() {
    with_live_daemon(
        "pane_read::a_read_names_its_pane_workspace_and_tab_and_echoes_the_request",
        async |app| {
            let script = "printf 'alpha\\nbeta\\ngamma\\n'";
            let (workspace_id, pane) = printed(app, script, "gamma").await;
            let wire = json!({"type": "pane.read", "pane_id": pane.pane_id, "source": "visible"});
            let read = read_of(answer(app, wire).await);
            assert_eq!(read.pane_id, pane.pane_id);
            assert_eq!(read.workspace_id, workspace_id);
            assert_eq!(read.tab_id, pane.tab_id);
            assert_eq!(
                (read.source, read.format),
                (ReadSource::Visible, ReadFormat::Text)
            );
            assert_eq!((read.revision, read.truncated), (0, false));
            assert!(read.text.contains("alpha\nbeta\ngamma"), "{:?}", read.text);
        },
    );
}

#[test]
fn a_line_limit_clips_the_visible_text_to_its_last_lines() {
    with_live_daemon(
        "pane_read::a_line_limit_clips_the_visible_text_to_its_last_lines",
        async |app| {
            let script = "printf 'alpha\\nbeta\\ngamma\\n'";
            let (_, pane) = printed(app, script, "gamma").await;
            let two = read_of(answer(app, read(&pane, "visible", Some(2), "text")).await);
            assert_eq!(two.text.lines().count(), 2, "{:?}", two.text);
            assert!(two.truncated);
            let none = read_of(answer(app, read(&pane, "visible", Some(0), "text")).await);
            assert_eq!((none.text.as_str(), none.truncated), ("", true));
            let all = read_of(answer(app, read(&pane, "visible", None, "text")).await);
            assert!(!all.truncated);
            assert!(all.text.contains("alpha"), "{:?}", all.text);
        },
    );
}

#[test]
fn recent_reads_report_when_older_rows_are_omitted() {
    with_live_daemon(
        "pane_read::recent_reads_report_when_older_rows_are_omitted",
        async |app| {
            let script = "seq 0 39 | sed 's/^/line /'";
            let (_, pane) = printed(app, script, "line 39").await;
            for source in ["recent", "recent_unwrapped"] {
                let few = read_of(answer(app, read(&pane, source, Some(2), "text")).await);
                assert!(few.text.contains("line 39"), "{source}: {:?}", few.text);
                assert!(!few.text.contains("line 10"), "{source}: {:?}", few.text);
                assert!(few.truncated, "{source}");
                let many = read_of(answer(app, read(&pane, source, Some(1000), "text")).await);
                assert!(many.text.contains("line 0"), "{source}: {:?}", many.text);
                assert!(!many.truncated, "{source}");
            }
            let detection = read_of(answer(app, read(&pane, "detection", Some(3), "text")).await);
            assert!(detection.text.contains("line 39"), "{:?}", detection.text);
        },
    );
}

#[test]
fn ansi_reads_keep_the_styling_that_text_reads_drop() {
    with_live_daemon(
        "pane_read::ansi_reads_keep_the_styling_that_text_reads_drop",
        async |app| {
            let script = "printf '\\033[31mred\\033[0m\\n'";
            let (_, pane) = printed(app, script, "red").await;
            let plain = read_of(answer(app, read(&pane, "recent", Some(100), "text")).await);
            assert!(plain.text.contains("red"), "{:?}", plain.text);
            assert!(!plain.text.contains('\u{1b}'), "{:?}", plain.text);
            let styled = read_of(answer(app, read(&pane, "recent", Some(100), "ansi")).await);
            assert!(styled.text.contains("red") && styled.text.contains("\u{1b}["));
            assert_eq!(styled.format, ReadFormat::Ansi);
        },
    );
}

#[test]
fn visible_ansi_reads_keep_the_styling_that_visible_text_reads_drop() {
    with_live_daemon(
        "pane_read::visible_ansi_reads_keep_the_styling_that_visible_text_reads_drop",
        async |app| {
            let script = "printf '\\033[31mred\\033[0m\\n'";
            let (_, pane) = printed(app, script, "red").await;
            let plain = read_of(answer(app, read(&pane, "visible", None, "text")).await);
            assert!(plain.text.contains("red") && !plain.text.contains('\u{1b}'));
            let styled = read_of(answer(app, read(&pane, "visible", None, "ansi")).await);
            assert!(styled.text.contains("red"), "{:?}", styled.text);
            assert!(styled.text.contains("\u{1b}["), "{:?}", styled.text);
        },
    );
}

#[test]
fn unwrapped_reads_join_the_soft_wrapped_rows_that_physical_reads_keep_apart() {
    with_live_daemon(
        "pane_read::unwrapped_reads_join_the_soft_wrapped_rows_that_physical_reads_keep_apart",
        async |app| {
            let script = "printf '\\033[31m'; printf '%0600d' 0; printf '\\033[0m\\nEND\\n'";
            let (_, pane) = printed(app, script, "END").await;
            let joined = "0".repeat(600);
            for format in ["text", "ansi"] {
                let physical = read_of(answer(app, read(&pane, "recent", Some(100), format)).await);
                assert!(
                    !physical.text.contains(&joined),
                    "{format}: {:?}",
                    physical.text
                );
                let logical =
                    read_of(answer(app, read(&pane, "recent_unwrapped", Some(100), format)).await);
                assert!(
                    logical.text.contains(&joined),
                    "{format}: {:?}",
                    logical.text
                );
                assert_eq!(
                    logical.text.contains('\u{1b}'),
                    format == "ansi",
                    "{format}"
                );
            }
        },
    );
}

#[test]
fn a_recent_read_without_a_line_count_returns_the_last_eighty_rows() {
    with_live_daemon(
        "pane_read::a_recent_read_without_a_line_count_returns_the_last_eighty_rows",
        async |app| {
            let script = "seq 1 200 | sed 's/^/row /'";
            let (_, pane) = printed(app, script, "row 200").await;
            for (source, format) in [
                ("recent", "text"),
                ("recent", "ansi"),
                ("recent_unwrapped", "text"),
                ("recent_unwrapped", "ansi"),
            ] {
                let omitted = read_of(answer(app, read(&pane, source, None, format)).await);
                let eighty = read_of(answer(app, read(&pane, source, Some(80), format)).await);
                assert_eq!(omitted.text, eighty.text, "{source} {format}");
                assert!(omitted.truncated, "{source} {format}");
                for lines in [79, 81] {
                    let other =
                        read_of(answer(app, read(&pane, source, Some(lines), format)).await);
                    assert_ne!(omitted.text, other.text, "{source} {format} {lines}");
                }
            }
        },
    );
}

#[test]
fn a_read_asks_for_at_most_one_thousand_lines() {
    with_live_daemon(
        "pane_read::a_read_asks_for_at_most_one_thousand_lines",
        async |app| {
            let script = "seq 1 1200 | sed 's/^/row /'";
            let (_, pane) = printed(app, script, "row 1200").await;
            for (source, format) in [
                ("recent", "text"),
                ("recent", "ansi"),
                ("recent_unwrapped", "text"),
                ("recent_unwrapped", "ansi"),
            ] {
                let capped = read_of(answer(app, read(&pane, source, Some(1001), format)).await);
                let limit = read_of(answer(app, read(&pane, source, Some(1000), format)).await);
                assert_eq!(capped.text, limit.text, "{source} {format}");
                assert!(capped.truncated, "{source} {format}");
                assert!(!capped.text.contains("row 100\n"), "{source} {format}");
            }
        },
    );
}

#[test]
fn a_pane_that_cannot_be_read_is_refused() {
    let mut app = daemon_with_workspaces(&["/one"]);
    for pane in ["w1:p1", "w1:p9"] {
        let wire = json!({"type": "pane.read", "pane_id": pane, "source": "visible"});
        assert_eq!(
            request(&mut app, wire),
            failure("pane_not_found", &format!("pane {pane} not found"))
        );
    }
}
