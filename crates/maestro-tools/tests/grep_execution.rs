//! Content search through the public tool factories.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
#[cfg(unix)]
use crate::read_support::Fixture;
#[cfg(unix)]
use crate::runtime;
use maestro_tools::{GrepToolOptions, ToolsManager, create_grep_tool_definition};
use std::sync::Arc;

#[test]
fn grep_definition_keeps_schema_metadata_and_root_exports() {
    let tool = create_grep_tool_definition::<()>(
        "/work".into(),
        GrepToolOptions::default(),
        Arc::new(ToolsManager::new("/unused".into(), None)),
    );
    let source: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/grep_cases.json")).unwrap();
    assert_eq!(
        serde_json::json!({"name":tool.definition.name,"label":tool.label,"description":tool.definition.description,"parameters":tool.definition.parameters,"promptSnippet":tool.prompt_snippet}),
        source["cases"][0]["expected"]
    );
    assert!(
        tool.prompt_guidelines.is_none()
            && tool.prepare_arguments.is_none()
            && tool.render_shell.is_none()
            && tool.execution_mode.is_none()
    );
    assert_eq!(
        serde_json::to_value(maestro_tools::search::grep::GrepToolDetails {
            truncation: None,
            match_limit_reached: None,
            lines_truncated: None
        })
        .unwrap(),
        serde_json::json!({})
    );
}

#[cfg(unix)]
#[test]
fn grep_supplied_operations_retain_native_search() {
    crate::grep_support::check("grep_supplied_operations_retain_native_search");
}

#[cfg(unix)]
#[test]
fn grep_resolves_paths_at_execution_after_acquisition() {
    crate::grep_support::check("grep_resolves_paths_at_execution_after_acquisition");
}

#[cfg(unix)]
#[test]
fn grep_native_arguments_keep_option_order_and_delimiter() {
    crate::grep_support::check("grep_native_arguments_keep_option_order_and_delimiter");
}

#[cfg(unix)]
#[test]
fn grep_limits_clamp_without_rounding_labels() {
    crate::grep_support::check("grep_limits_clamp_without_rounding_labels");
}

#[cfg(unix)]
#[test]
fn grep_default_limit_counts_events_in_order() {
    crate::grep_support::check("grep_default_limit_counts_events_in_order");
}

#[cfg(unix)]
#[test]
fn grep_json_events_keep_framing_order_and_count() {
    crate::grep_support::check("grep_json_events_keep_framing_order_and_count");
}

#[cfg(unix)]
#[test]
fn grep_path_projection_keeps_file_identity() {
    crate::grep_support::check("grep_path_projection_keeps_file_identity");
}

#[cfg(unix)]
#[test]
fn grep_context_bounds_and_numeric_policy() {
    crate::grep_support::check("grep_context_bounds_and_numeric_policy");
}

#[cfg(unix)]
#[test]
fn grep_context_preserves_overlaps_and_caches_reads() {
    crate::grep_support::check("grep_context_preserves_overlaps_and_caches_reads");
}

#[cfg(unix)]
#[test]
fn grep_reread_fallback_is_cached_and_lossy() {
    crate::grep_support::check("grep_reread_fallback_is_cached_and_lossy");
}

#[cfg(unix)]
#[test]
fn grep_inline_text_normalization_matches_protocol() {
    crate::grep_support::check("grep_inline_text_normalization_matches_protocol");
}

#[cfg(unix)]
#[test]
fn grep_line_clipping_counts_unicode_scalars() {
    crate::grep_support::check("grep_line_clipping_counts_unicode_scalars");
}

#[cfg(unix)]
#[test]
fn grep_byte_selection_has_no_extra_line_cap() {
    crate::grep_support::check("grep_byte_selection_has_no_extra_line_cap");
}

#[cfg(unix)]
#[test]
fn grep_count_byte_and_clip_notices_keep_order() {
    crate::grep_support::check("grep_count_byte_and_clip_notices_keep_order");
}

#[cfg(unix)]
#[test]
fn grep_failures_keep_acquisition_path_and_exit_precedence() {
    use crate::{grep_support as g, read_support};
    if maestro_test_process::selection().is_child() {
        let root = read_support::Scratch::new().unwrap();
        let case = g::cases("grep_failures_keep_acquisition_path_and_exit_precedence")
            .into_iter()
            .find(|c| c.id == "unavailable")
            .unwrap();
        assert_eq!(case.op, "controlled");
        assert_eq!(
            case.input,
            serde_json::json!({"unavailable":true,"fail":"stat"})
        );
        let manager = crate::find_support::manager(
            root.text(),
            &Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        );
        let tool =
            maestro_tools::create_grep_tool(root.text(), GrepToolOptions::default(), manager);
        let error = runtime::run((tool.execute)(
            "call".into(),
            serde_json::json!({"pattern":"hit"}),
            None,
            None,
        ))
        .unwrap_err();
        assert_eq!(error.message, case.expected["error"]);
        assert_eq!(case.expected["operations"], serde_json::json!([]));
        assert_eq!(case.expected["updates"], serde_json::json!(0));
        return;
    }
    for case in g::cases("grep_failures_keep_acquisition_path_and_exit_precedence")
        .into_iter()
        .filter(|c| c.id != "unavailable")
    {
        g::controlled(&case);
    }
    let root = read_support::Scratch::new().unwrap();
    assert!(!read_support::isolated_os(
        "grep_failures_keep_acquisition_path_and_exit_precedence",
        "unavailable",
        &[
            ("PATH", Some(root.path().as_os_str())),
            ("MAESTRO_OFFLINE", Some(std::ffi::OsStr::new("1")))
        ]
    ));
}

#[cfg(unix)]
#[test]
fn grep_single_file_includes_basename() {
    let root = crate::grep_support::tree();
    for c in crate::grep_support::cases("grep_single_file_includes_basename") {
        crate::grep_support::check_native(&c, &root);
    }
}

#[cfg(unix)]
#[test]
fn grep_global_limit_includes_requested_context() {
    let root = crate::grep_support::tree();
    for c in crate::grep_support::cases("grep_global_limit_includes_requested_context") {
        crate::grep_support::check_native(&c, &root);
    }
}

#[cfg(unix)]
#[test]
fn grep_flag_like_pattern_never_runs_commands() {
    let root = crate::grep_support::tree();
    crate::find_support::executable(
        &root,
        "payload.sh",
        &format!(
            "#!/bin/sh\necho executed > '{}'\ncat \"$1\"\n",
            root.path().join("injection-marker").display()
        ),
    );
    for c in crate::grep_support::cases("grep_flag_like_pattern_never_runs_commands") {
        crate::grep_support::check_native(&c, &root);
    }
}

#[cfg(unix)]
#[test]
fn grep_native_files_and_protocol_bytes() {
    use crate::grep_support as g;
    let root = g::tree();
    let invalid_filename = g::add_invalid_filename(&root);
    for c in g::cases("grep_native_files_and_protocol_bytes") {
        if c.id != "invalid-filename" || invalid_filename {
            g::check_native(&c, &root);
        }
    }
    if invalid_filename {
        root.file("invalid-�", b"wrong\n");
        let result = g::native(
            &root,
            serde_json::json!({"pattern":"needle","glob":"invalid-*","context":1}),
        )
        .unwrap();
        assert_eq!(
            result["content"][0]["text"],
            "invalid-�:1: needle\ninvalid-�-2- "
        );
    }
    root.file("invalid-content", b"\xef\xbb\xbfhit\xff\n");
    let result = g::native(
        &root,
        serde_json::json!({"pattern":"hit","path":"invalid-content","context":1}),
    )
    .unwrap();
    assert_eq!(
        result["content"][0]["text"],
        "invalid-content:1: \u{feff}hit�\ninvalid-content-2- "
    );
}

#[cfg(unix)]
#[test]
fn grep_native_search_options_reach_ripgrep() {
    use crate::grep_support as g;
    let root = g::tree();
    for c in g::cases("grep_native_search_options_reach_ripgrep") {
        g::check_native(&c, &root);
    }
    root.file("selected.rs", b"unique-hit\n");
    root.file("excluded.txt", b"unique-hit\n");
    assert_eq!(
        g::native(
            &root,
            serde_json::json!({"pattern":"unique-hit","glob":"*.rs"})
        )
        .unwrap()["content"][0]["text"],
        "selected.rs:1: unique-hit"
    );
}

/// The lines of a result's first text block, sorted, since search order is not recorded.
#[cfg(unix)]
fn sorted_lines(result: &serde_json::Value) -> Vec<&str> {
    let mut lines: Vec<_> = result["content"][0]["text"]
        .as_str()
        .unwrap()
        .lines()
        .collect();
    lines.sort_unstable();
    lines
}

#[cfg(unix)]
#[test]
fn grep_native_nested_ignores_hidden_and_metadata() {
    use crate::grep_support as g;
    let root = g::tree();
    let case = g::cases("grep_native_nested_ignores_hidden_and_metadata").remove(0);
    assert_eq!(case.op, "native-rg");
    assert_eq!(
        case.input,
        serde_json::json!({"params":{"pattern":"needle"}})
    );
    let invalid_filename = g::add_invalid_filename(&root);
    let mut expected = sorted_lines(&case.expected["result"]);
    if !invalid_filename {
        expected.retain(|line| *line != "invalid-�:1: needle");
    }
    for worktree in [false, true] {
        if worktree {
            assert!(
                std::process::Command::new("git")
                    .args(["init", "-q"])
                    .arg(root.path())
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let result = g::native(&root, serde_json::json!({"pattern":"needle"})).unwrap();
        assert!(result.get("details").is_none());
        assert_eq!(sorted_lines(&result), expected);
        let filtered = g::native(
            &root,
            serde_json::json!({"pattern":"needle","glob":"**/local.txt"}),
        )
        .unwrap();
        assert_eq!(
            sorted_lines(&filtered),
            ["a/local.txt:1: needle", "b/local.txt:1: needle"]
        );
        root.file(".git/meta.txt", b"needle");
        assert_eq!(
            g::native(
                &root,
                serde_json::json!({"pattern":"needle","path":".git/meta.txt"})
            )
            .unwrap()["content"][0]["text"],
            "meta.txt:1: needle"
        );
        assert_eq!(
            g::native(
                &root,
                serde_json::json!({"pattern":"needle","glob":".git/**"})
            )
            .unwrap()["content"][0]["text"],
            "No matches found"
        );
    }
}

#[cfg(unix)]
#[test]
fn grep_cancellation_drops_pending_operations() {
    use crate::{grep_support as g, read_support};
    use maestro_models::Cancellation;
    if maestro_test_process::selection().is_child() {
        cancel_pending_acquisition();
        return;
    }
    for read in [false, true] {
        let root = read_support::Scratch::new().unwrap();
        g::producer(&root, &g::Input::default());
        let operations = Arc::new(g::PendingOperations {
            read,
            started: Cancellation::new(),
            dropped: Cancellation::new(),
        });
        let manager = crate::find_support::manager(
            root.text(),
            &Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        );
        let tool = maestro_tools::create_grep_tool(
            root.text(),
            GrepToolOptions {
                operations: Some(operations.clone()),
            },
            manager,
        );
        runtime::run(async {
            let signal = Cancellation::new();
            let execution = (tool.execute)(
                "call".into(),
                serde_json::json!({"pattern":"hit","context":1}),
                Some(signal.clone()),
                None,
            );
            tokio::pin!(execution);
            tokio::select! {r=&mut execution=>panic!("settled before pending effect: {}",r.is_ok()),()=operations.started.cancelled()=>{}}
            signal.abort();
            assert_eq!(execution.await.unwrap_err().message, "Operation aborted");
            assert!(operations.dropped.is_aborted());
        });
    }
    let root = read_support::Scratch::new().unwrap();
    assert!(!read_support::isolated_os(
        "grep_cancellation_drops_pending_operations",
        "acquisition",
        &[
            ("PATH", Some(root.path().as_os_str())),
            ("MAESTRO_OFFLINE", None)
        ]
    ));
}

#[cfg(unix)]
#[test]
fn grep_cancellation_stops_live_child() {
    use crate::{grep_support as g, read_support};
    use maestro_models::Cancellation;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    runtime::run(async {
        for limited in [false, true] {
            let root = read_support::Scratch::new().unwrap();
            root.file("file.txt", b"hit\n");
            let event=serde_json::json!({"type":"match","data":{"path":{"text":root.path().join("file.txt").to_str().unwrap()},"line_number":1,"lines":{"text":"hit\n"}}}).to_string()+"\n";
            let listener = g::session(
                &root,
                &format!(
                    "signal.pthread_sigmask(signal.SIG_BLOCK,{{signal.SIGTERM}})\ns.sendall(b'R')\nassert s.recv(1)==b'x'\nos.write(1,{}.encode())\n{}\nsignal.sigwait({{signal.SIGTERM}})\ns.sendall(b'T')\nassert s.recv(1)==b'x'\nsys.exit(9)",
                    serde_json::json!(event),
                    if limited { "" } else { "s.sendall(b'S')" }
                ),
            );
            let signal = Cancellation::new();
            let execution = g::invocation(
                &root,
                serde_json::json!({"pattern":"hit","limit":if limited {1}else{100}}),
                Some(signal.clone()),
            );
            tokio::pin!(execution);
            let (mut socket, _) = tokio::select! {r=&mut execution=>panic!("ended before readiness: {}",r.is_ok()),s=listener.accept()=>s.unwrap()};
            let mut byte = [0];
            socket.read_exact(&mut byte).await.unwrap();
            assert_eq!(byte, b"R"[..]);
            socket.write_all(b"x").await.unwrap();
            if limited {
                tokio::select! {r=&mut execution=>panic!("ended before limit termination: {}",r.is_ok()),r=socket.read_exact(&mut byte)=>{r.unwrap();}}
                assert_eq!(byte, b"T"[..]);
            } else {
                tokio::select! {r=&mut execution=>panic!("ended before start acknowledgement: {}",r.is_ok()),r=socket.read_exact(&mut byte)=>{r.unwrap();}}
                assert_eq!(byte, b"S"[..]);
            }
            signal.abort();
            if !limited {
                tokio::select! {r=&mut execution=>panic!("ended before cancellation termination: {}",r.is_ok()),r=socket.read_exact(&mut byte)=>{r.unwrap();}}
                assert_eq!(byte, b"T"[..]);
            }
            socket.write_all(b"x").await.unwrap();
            assert_eq!(execution.await.unwrap_err().message, "Operation aborted");
            assert_eq!(socket.read(&mut byte).await.unwrap(), 0);
        }
    });
}

#[cfg(unix)]
#[test]
fn grep_limit_terminates_before_later_results() {
    use crate::{grep_support as g, read_support};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    runtime::run(async {
        let root = read_support::Scratch::new().unwrap();
        root.file("first", b"hit\nafter\n");
        let event=serde_json::json!({"type":"match","data":{"path":{"text":root.path().join("first").to_str().unwrap()},"line_number":1}}).to_string()+"\n";
        let listener = g::session(
            &root,
            &format!(
                "signal.pthread_sigmask(signal.SIG_BLOCK,{{signal.SIGTERM}})\ns.sendall(b'R')\nassert s.recv(1)==b'x'\nos.write(1,{}.encode())\nsignal.sigwait({{signal.SIGTERM}})\ns.sendall(b'T')\nassert s.recv(1)==b'x'\nsys.exit(9)",
                serde_json::json!(event)
            ),
        );
        let execution = g::invocation(
            &root,
            serde_json::json!({"pattern":"hit","limit":1,"context":1}),
            None,
        );
        tokio::pin!(execution);
        let (mut socket, _) = tokio::select! {r=&mut execution=>panic!("ended before readiness: {}",r.is_ok()),s=listener.accept()=>s.unwrap()};
        let mut byte = [0];
        socket.read_exact(&mut byte).await.unwrap();
        assert_eq!(byte, b"R"[..]);
        socket.write_all(b"x").await.unwrap();
        tokio::select! {r=&mut execution=>panic!("ended before count termination: {}",r.is_ok()),r=socket.read_exact(&mut byte)=>{r.unwrap();}}
        assert_eq!(byte, b"T"[..]);
        socket.write_all(b"x").await.unwrap();
        let result = execution.await.unwrap();
        assert_eq!(
            serde_json::to_value(&result.content).unwrap()[0]["text"],
            "first:1: hit\nfirst-2- after\n\n[1 matches limit reached. Use limit=2 for more, or refine pattern]"
        );
        assert_eq!(
            result.details.unwrap(),
            serde_json::json!({"matchLimitReached":1.0})
        );
        assert_eq!(socket.read(&mut byte).await.unwrap(), 0);
    });
}

#[cfg(unix)]
#[test]
fn grep_stdout_framing_ignores_stderr() {
    use crate::{grep_support as g, read_support};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    runtime::run(async {
        for code in [0, 2] {
            let root = read_support::Scratch::new().unwrap();
            let event=serde_json::json!({"type":"match","data":{"path":{"text":root.path().join("file.txt").to_str().unwrap()},"line_number":1,"lines":{"text":"é\n"}}}).to_string();
            let split = event.as_bytes().iter().position(|b| *b == 0xc3).unwrap() + 1;
            let stderr_event=b"{\"type\":\"match\",\"data\":{\"path\":{\"text\":\"wrong\"},\"line_number\":9,\"lines\":{\"text\":\"wrong\"}}}\n";
            let chunks = if code == 0 {
                vec![
                    (1, event.as_bytes()[..split].to_vec()),
                    (2, stderr_event.to_vec()),
                    (
                        1,
                        [
                            event.as_bytes()[split..].to_vec(),
                            b"\r\n\n{bad\n".to_vec(),
                            event.as_bytes().to_vec(),
                        ]
                        .concat(),
                    ),
                ]
            } else {
                vec![
                    (2, b"\xef\xbb\xbf d\xc3".to_vec()),
                    (2, b"\xa9nied \r\n".to_vec()),
                ]
            };
            let writes = chunks.iter().fold(String::new(), |mut script,(fd,bytes)| {
                use std::fmt::Write;
                let encoded=base64::Engine::encode(&base64::prelude::BASE64_STANDARD,bytes);
                writeln!(script,"s.sendall(b'R')\nassert s.recv(1)==b'x'\nos.write({fd},__import__('base64').b64decode('{encoded}'))").unwrap();
                script
            });
            let listener = g::session(&root, &format!("{writes}\nsys.exit({code})"));
            let execution = g::invocation(&root, serde_json::json!({"pattern":"hit"}), None);
            tokio::pin!(execution);
            let (mut socket, _) = tokio::select! {r=&mut execution=>panic!("ended before readiness: {}",r.is_ok()),s=listener.accept()=>s.unwrap()};
            for _ in &chunks {
                let mut byte = [0];
                tokio::select! {r=&mut execution=>panic!("ended before chunk: {}",r.is_ok()),r=socket.read_exact(&mut byte)=>{r.unwrap();}}
                assert_eq!(byte, b"R"[..]);
                socket.write_all(b"x").await.unwrap();
            }
            let result = execution.await;
            if code == 0 {
                let result = result.unwrap();
                assert_eq!(
                    serde_json::to_value(&result.content).unwrap()[0]["text"],
                    "file.txt:1: é\nfile.txt:1: é"
                );
                assert!(result.details.is_none());
            } else {
                assert_eq!(result.unwrap_err().message, "dénied");
            }
        }
    });
}

/// Cancel the in-flight acquisition inside an isolated child.
#[cfg(unix)]
fn cancel_pending_acquisition() {
    use crate::{grep_support as g, read_support};
    use maestro_models::{Cancellation, FetchError};
    let started = Cancellation::new();
    let dropped = Cancellation::new();
    let entered = started.clone();
    let guard = g::Dropped(dropped.clone());
    let fetch: maestro_models::Fetch = Arc::new(move |_| {
        let entered = entered.clone();
        let guard = g::Dropped(guard.0.clone());
        Box::pin(async move {
            let _guard = guard;
            entered.abort();
            std::future::pending::<Result<_, FetchError>>().await
        })
    });
    let root = read_support::Scratch::new().unwrap();
    let manager = Arc::new(ToolsManager::new(root.text(), Some(fetch)));
    runtime::run(async {
        let signal = Cancellation::new();
        let tool =
            maestro_tools::create_grep_tool(root.text(), GrepToolOptions::default(), manager);
        let execution = (tool.execute)(
            "call".into(),
            serde_json::json!({"pattern":"hit"}),
            Some(signal.clone()),
            None,
        );
        tokio::pin!(execution);
        tokio::select! {r=&mut execution=>panic!("settled before acquisition: {}",r.is_ok()),()=started.cancelled()=>{}}
        signal.abort();
        assert_eq!(execution.await.unwrap_err().message, "Operation aborted");
        assert!(dropped.is_aborted());
    });
}
