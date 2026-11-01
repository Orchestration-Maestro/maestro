//! File finding through the public tool definition.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::read_support::Fixture;
#[cfg(unix)]
use crate::runtime;
use crate::{display_support, find_support, read_support};
use maestro_tools::{FindToolOptions, ToolsManager, create_find_tool_definition};
use std::sync::Arc;
#[test]
fn find_definition_preserves_schema_and_metadata() {
    let tool = create_find_tool_definition::<()>(
        "/work".into(),
        FindToolOptions::default(),
        Arc::new(ToolsManager::new("/unused".into(), None)),
    );
    let case = find_support::cases("find_definition_preserves_schema_and_metadata").remove(0);
    assert_eq!(case.op, "definition");
    assert_eq!(case.input, serde_json::json!({}));
    assert_eq!(
        serde_json::json!({"name":tool.definition.name,"label":tool.label,"description":tool.definition.description,"parameters":tool.definition.parameters,"promptSnippet":tool.prompt_snippet}),
        case.expected
    );
    assert!(
        tool.prompt_guidelines.is_none()
            && tool.prepare_arguments.is_none()
            && tool.render_shell.is_none()
            && tool.execution_mode.is_none()
    );
    assert_eq!(
        serde_json::to_value(maestro_tools::FindToolDetails {
            truncation: None,
            result_limit_reached: None
        })
        .unwrap(),
        serde_json::json!({})
    );
    check_wrapped_decoding();
}

/// Exercise the existing wrapper decoder independently of schema declarations.
fn check_wrapped_decoding() {
    // Schema declarations and wrapped decoding are deliberately separate routes.
    for (params, accepted) in [
        (serde_json::json!({"pattern":"","extra":true}), true),
        (
            serde_json::json!({"pattern":"*","path":null,"limit":null}),
            true,
        ),
        (serde_json::json!(["*", null, null]), true),
        (serde_json::json!({}), false),
        (serde_json::json!({"pattern":null}), false),
        (serde_json::json!({"pattern":1}), false),
        (serde_json::json!({"pattern":"*","path":42}), false),
        (serde_json::json!({"pattern":"*","limit":"2"}), false),
    ] {
        let record = display_support::Case {
            test: String::new(),
            id: String::new(),
            op: "custom".into(),
            input: serde_json::json!({"wrapped":true,"params":params,"results":[]}),
            expected: serde_json::Value::Null,
        };
        let observed = find_support::custom(&record);
        assert_eq!(observed.get("error").is_none(), accepted);
        if accepted {
            assert_eq!(observed["calls"].as_array().unwrap().len(), 2);
            assert_eq!(
                observed["result"]["content"][0]["text"],
                "No files found matching pattern"
            );
        } else {
            assert_eq!(observed["calls"], serde_json::json!([]));
        }
    }
}

#[test]
fn find_factory_forwards_custom_operations_without_native_work() {
    find_support::check_custom("find_factory_forwards_custom_operations_without_native_work");
}

/// A completed custom route must not even run an installed PATH discovery probe.
#[cfg(unix)]
#[test]
fn find_custom_route_never_probes_native_programs() {
    let root = read_support::Scratch::new().unwrap();
    let marker = root.path().join("probed");
    let test = "find_custom_route_never_probes_native_programs";
    if !maestro_test_process::selection().is_child() {
        let script = format!(
            "#!/usr/bin/python3\nfrom pathlib import Path\nPath({}).write_text('probed')\n",
            serde_json::json!(marker.to_str().unwrap())
        );
        for name in ["fd", "fdfind"] {
            find_support::executable(&root, name, &script);
        }
        assert!(
            std::process::Command::new(root.path().join("fd"))
                .arg("--version")
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "probed");
        std::fs::remove_file(&marker).unwrap();
        assert!(!read_support::isolated(
            test,
            "probe",
            &[("PATH", Some(root.text().as_str()))]
        ));
        assert!(!marker.exists(), "custom execution ran native discovery");
        return;
    }
    find_support::check_custom("find_factory_forwards_custom_operations_without_native_work");
}

#[test]
fn find_paths_resolve_at_execution() {
    find_support::check_custom("find_paths_resolve_at_execution");
}

/// Drive context and both separator spellings survive supplied-operation projection.
#[cfg(windows)]
#[test]
fn find_windows_results_keep_drive_context_and_directory_suffix() {
    let case = display_support::Case {
        test: String::new(),
        id: String::new(),
        op: "custom".into(),
        input: serde_json::json!({"cwd":"C:\\work", "results":["C:/work/sub/"]}),
        expected: serde_json::Value::Null,
    };
    assert_eq!(
        find_support::custom(&case)["result"]["content"][0]["text"],
        "sub/"
    );
    let current = std::env::current_dir().unwrap();
    let current = current.to_str().unwrap();
    if current.as_bytes().get(1) == Some(&b':') {
        let case = display_support::Case {
            input: serde_json::json!({"cwd":"\\work", "results":[format!("{}\\work\\a", &current[..2])]}),
            ..case
        };
        assert_eq!(
            find_support::custom(&case)["result"]["content"][0]["text"],
            "a"
        );
    }
}

#[test]
fn find_custom_limits_are_forwarded_without_slicing() {
    find_support::check_custom("find_custom_limits_are_forwarded_without_slicing");
}

#[test]
fn find_default_limit_and_count_notices() {
    find_support::check_custom("find_default_limit_and_count_notices");
}

#[test]
fn find_empty_custom_result_has_no_details() {
    find_support::check_custom("find_empty_custom_result_has_no_details");
}

#[test]
fn find_custom_path_conversion_preserves_identity_and_order() {
    find_support::check_custom("find_custom_path_conversion_preserves_identity_and_order");
}

#[test]
fn find_custom_errors_keep_precheck_precedence() {
    find_support::check_custom("find_custom_errors_keep_precheck_precedence");
}

#[test]
fn find_cancellation_wins_before_and_during_custom_work() {
    find_support::check_custom("find_cancellation_wins_before_and_during_custom_work");
}

#[test]
fn find_byte_budget_has_no_line_cap() {
    find_support::check_custom("find_byte_budget_has_no_line_cap");
}

#[test]
fn find_custom_count_and_byte_notices_keep_order() {
    find_support::check_custom("find_custom_count_and_byte_notices_keep_order");
}

#[cfg(unix)]
#[test]
fn find_native_arguments_preserve_glob_modes() {
    find_support::check_controlled("find_native_arguments_preserve_glob_modes");
}

#[cfg(unix)]
#[test]
fn find_native_limits_keep_number_spelling_and_notices() {
    find_support::check_controlled("find_native_limits_keep_number_spelling_and_notices");
}

#[cfg(unix)]
#[test]
fn find_native_failures_and_partial_output_follow_exit_rules() {
    find_support::check_controlled("find_native_failures_and_partial_output_follow_exit_rules");
}

#[cfg(unix)]
#[test]
fn find_native_path_records_preserve_names_and_directories() {
    find_support::check_controlled("find_native_path_records_preserve_names_and_directories");
    for name in [
        " leading ",
        "trailing ",
        "line\nbreak",
        "carriage\rname",
        "ending\\",
    ] {
        let root = read_support::Scratch::new().unwrap();
        root.file(name, b"");
        let result = find_support::native(&root, "*", Some(2.0)).unwrap();
        assert_eq!(find_support::text(&result), name);
        assert!(result.get("details").is_none());
    }
    let root = read_support::Scratch::new().unwrap();
    std::fs::create_dir(root.path().join("directory")).unwrap();
    assert_files(&root, "*", &["directory/"]);
}

#[cfg(unix)]
#[test]
fn find_native_count_and_byte_notices_keep_order() {
    find_support::check_controlled("find_native_count_and_byte_notices_keep_order");
}

#[cfg(unix)]
#[test]
fn find_resolution_observes_cwd_only_when_required() {
    if !read_support::isolated(
        "find_resolution_observes_cwd_only_when_required",
        "removed",
        &[],
    ) {
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    root.enter_removed("removed").unwrap();
    assert!(std::env::current_dir().is_err());
    find_support::check_custom("find_resolution_observes_cwd_only_when_required");
}

/// A tree with distinguishing basename and directory patterns.
fn glob_tree() -> read_support::Scratch {
    let root = read_support::Scratch::new().unwrap();
    for path in ["some/parent/child", "src/foo/bar"] {
        std::fs::create_dir_all(root.path().join(path)).unwrap();
    }
    for path in [
        "some/parent/child/file.ext",
        "some/parent/child/test.spec.ts",
        "src/foo/bar/example.spec.ts",
        "root.txt",
    ] {
        root.file(path, b"");
    }
    root
}
/// Compare a real traversal's file set without assuming producer scheduling order.
fn assert_files(root: &read_support::Scratch, pattern: &str, expected: &[&str]) {
    let result = find_support::native(root, pattern, None).unwrap();
    let paths: std::collections::BTreeSet<_> = find_support::text(&result).split('\n').collect();
    assert_eq!(paths, expected.iter().copied().collect());
    assert!(result.get("details").is_none());
}
#[test]
fn find_basename_glob_reaches_nested_files() {
    assert_files(
        &glob_tree(),
        "*.spec.ts",
        &[
            "some/parent/child/test.spec.ts",
            "src/foo/bar/example.spec.ts",
        ],
    );
}
#[test]
fn find_directory_glob_reaches_subtree() {
    assert_files(
        &glob_tree(),
        "some/parent/child/**",
        &[
            "some/parent/child/file.ext",
            "some/parent/child/test.spec.ts",
        ],
    );
}
#[test]
fn find_recursive_path_glob_reaches_children() {
    assert_files(
        &glob_tree(),
        "**/parent/child/*",
        &[
            "some/parent/child/file.ext",
            "some/parent/child/test.spec.ts",
        ],
    );
}
#[test]
fn find_nested_spec_glob_reaches_leaf() {
    assert_files(
        &glob_tree(),
        "src/**/*.spec.ts",
        &["src/foo/bar/example.spec.ts"],
    );
}
#[test]
fn find_native_includes_hidden_nonmetadata_and_dependencies() {
    for worktree in [false, true] {
        let root = read_support::Scratch::new().unwrap();
        for dir in [".secret", "node_modules"] {
            std::fs::create_dir(root.path().join(dir)).unwrap();
        }
        if worktree {
            std::fs::create_dir(root.path().join(".git")).unwrap();
            root.file(".git/meta.txt", b"");
        }
        root.file(".gitignore", b"ignored.txt\n");
        for path in [
            ".secret/hidden.txt",
            "node_modules/dependency.txt",
            "ignored.txt",
            "kept.txt",
        ] {
            root.file(path, b"");
        }
        assert_files(
            &root,
            "**/*.txt",
            &[
                ".secret/hidden.txt",
                "node_modules/dependency.txt",
                "kept.txt",
            ],
        );
    }
}
#[test]
fn find_native_reports_glob_parse_errors() {
    let root = read_support::Scratch::new().unwrap();
    assert_eq!(
        find_support::native(&root, "[", None).unwrap_err(),
        "[fd error]: error parsing glob '[': unclosed character class; missing ']'"
    );
}
#[test]
fn find_flag_like_pattern_is_literal() {
    let root = read_support::Scratch::new().unwrap();
    assert_eq!(
        find_support::text(&find_support::native(&root, "--help", None).unwrap()),
        "No files found matching pattern"
    );
    root.file("--help", b"");
    assert_files(&root, "--help", &["--help"]);
}
#[test]
fn find_zero_native_limit_is_unlimited_without_warning() {
    let root = read_support::Scratch::new().unwrap();
    for n in 0..1005 {
        root.file(&format!("file-{n}.txt"), b"");
    }
    let result = find_support::native(&root, "*.txt", Some(0.0)).unwrap();
    let paths: std::collections::BTreeSet<_> = find_support::text(&result)
        .split('\n')
        .map(str::to_owned)
        .collect();
    assert_eq!(paths, (0..1005).map(|n| format!("file-{n}.txt")).collect());
    assert!(result.get("details").is_none());
}
#[test]
fn find_native_invalid_numeric_limits_remain_fd_errors() {
    let root = read_support::Scratch::new().unwrap();
    root.file("a.txt", b"");
    for (limit, expected) in [
        (
            -1.0,
            "error: a value is required for '--max-results <count>' but none was supplied\n\nFor more information, try '--help'.",
        ),
        (
            0.5,
            "error: invalid value '0.5' for '--max-results <count>': invalid digit found in string\n\nFor more information, try '--help'.",
        ),
        (
            1.5,
            "error: invalid value '1.5' for '--max-results <count>': invalid digit found in string\n\nFor more information, try '--help'.",
        ),
    ] {
        assert_eq!(
            find_support::native(&root, "*.txt", Some(limit)).unwrap_err(),
            expected
        );
    }
}
#[cfg(unix)]
#[test]
fn find_native_symlinks_and_invalid_utf8_keep_fd_results() {
    use std::os::unix::{ffi::OsStrExt, fs::symlink};
    let root = read_support::Scratch::new().unwrap();
    std::fs::create_dir(root.path().join("directory")).unwrap();
    root.file("file", b"");
    for (target, name) in [
        ("directory", "linked-dir"),
        ("file", "linked-file"),
        ("missing", "dangling-link"),
    ] {
        symlink(target, root.path().join(name)).unwrap();
    }
    assert_files(
        &root,
        "*link*",
        &["linked-dir", "linked-file", "dangling-link"],
    );
    let invalid = std::ffi::OsStr::from_bytes(b"invalid-\xff");
    if !root.keeps_distinct(&[invalid]).unwrap() {
        eprintln!(
            "the lossy name needs a host that stores the bytes {:?}",
            invalid.as_bytes()
        );
        return;
    }
    std::fs::write(root.path().join(invalid), b"").unwrap();
    assert_files(&root, "invalid-*", &["invalid-�"]);
}
/// Native nested ignore rules with both repository states.
fn nested_ignore(deep: bool) {
    for worktree in [false, true] {
        let root = read_support::Scratch::new().unwrap();
        for dir in ["a/deep", "b"] {
            std::fs::create_dir_all(root.path().join(dir)).unwrap();
        }
        if worktree {
            std::fs::create_dir(root.path().join(".git")).unwrap();
        }
        root.file("a/.gitignore", b"ignored.txt\n");
        for path in [
            "a/ignored.txt",
            "a/kept.txt",
            "b/ignored.txt",
            "b/kept.txt",
            "root.txt",
        ] {
            root.file(path, b"");
        }
        if deep {
            root.file("a/deep/.gitignore", b"secret.txt\n");
            for name in ["ignored.txt", "secret.txt", "kept.txt"] {
                root.file(&format!("a/deep/{name}"), b"");
            }
            assert_files(
                &root,
                "**/*.txt",
                &[
                    "a/deep/kept.txt",
                    "a/kept.txt",
                    "b/ignored.txt",
                    "b/kept.txt",
                    "root.txt",
                ],
            );
        } else {
            assert_files(
                &root,
                "**/*.txt",
                &["a/kept.txt", "b/ignored.txt", "b/kept.txt", "root.txt"],
            );
        }
    }
}
#[test]
fn find_nested_ignore_keeps_siblings_visible() {
    nested_ignore(false);
}
#[test]
fn find_deeper_ignore_stays_in_its_subtree() {
    nested_ignore(true);
}

#[cfg(unix)]
#[test]
fn find_native_abort_terminates_child_without_waiting_for_exit() {
    use futures_util::FutureExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let root = read_support::Scratch::new().unwrap();
    let socket_path = root.path().join("native.sock");
    let script = format!(
        "#!/usr/bin/python3\nimport socket,signal\ns=socket.socket(socket.AF_UNIX)\ns.connect({})\nsignal.pthread_sigmask(signal.SIG_BLOCK,{{signal.SIGTERM}})\ns.sendall(b'ready')\nsignal.sigwait({{signal.SIGTERM}})\ns.sendall(b'term')\ns.recv(1)\n",
        serde_json::json!(socket_path.to_str().unwrap())
    );
    find_support::executable(&root, "fd", &script);
    runtime::run(async {
        let listener = tokio::net::UnixListener::bind(socket_path).unwrap();
        let signal = maestro_models::Cancellation::new();
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let tool = maestro_tools::create_find_tool(
            root.text(),
            FindToolOptions::default(),
            find_support::manager(root.text(), &counter),
        );
        let mut execution = Box::pin((tool.execute)(
            "call".into(),
            serde_json::json!({"pattern":"*"}),
            Some(signal.clone()),
            None,
        ));
        let (mut socket, _) = tokio::select! {result=&mut execution=>panic!("ended before readiness: {}",result.is_ok()),connection=listener.accept()=>connection.unwrap()};
        let mut ready = [0; 5];
        socket.read_exact(&mut ready).await.unwrap();
        assert_eq!(&ready, b"ready");
        signal.abort();
        let error = execution
            .as_mut()
            .now_or_never()
            .expect("abort settles without child exit")
            .err()
            .unwrap();
        assert_eq!(error.message, "Operation aborted");
        let mut term = [0; 4];
        socket.read_exact(&mut term).await.unwrap();
        assert_eq!(&term, b"term");
        socket.write_all(b"x").await.unwrap();
        assert_eq!(socket.read(&mut [0]).await.unwrap(), 0);
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 0);
    });
}

#[cfg(unix)]
#[test]
fn find_acquisition_is_silent_and_abort_prevents_launch() {
    let test = "find_acquisition_is_silent_and_abort_prevents_launch";
    let root = read_support::Scratch::new().unwrap();
    if !maestro_test_process::selection().is_child() {
        assert_silent_child(
            test,
            read_support::isolated_os_output(
                test,
                "no-path",
                &[
                    ("PATH", Some(root.path().as_os_str())),
                    ("MAESTRO_OFFLINE", Some(std::ffi::OsStr::new("0"))),
                ],
            ),
        );
        let sock = root.path().join("probe.sock");
        find_support::executable(
            &root,
            "fd",
            &format!(
                "#!/usr/bin/python3\nimport socket\ns=socket.socket(socket.AF_UNIX)\ns.connect({})\ns.sendall(b'ready')\ns.recv(1)\n",
                serde_json::json!(sock.to_str().unwrap())
            ),
        );
        assert_silent_child(
            test,
            read_support::isolated_os_output(
                test,
                "probe",
                &[
                    ("PATH", Some(root.path().as_os_str())),
                    ("MAESTRO_FIND_PROBE", Some(root.path().as_os_str())),
                ],
            ),
        );
        return;
    }
    runtime::run(async {
        if maestro_test_process::selection().runs_case("probe") {
            abort_discovery().await;
        } else {
            unavailable_and_pre_abort(&root).await;
            abort_download(&root).await;
        }
    });
}
/// Ignore only harness lines, not the successful child's acquisition output.
#[cfg(unix)]
fn assert_silent_child(test: &str, output: std::process::Output) {
    let success = format!(
        "test {} ... ok",
        maestro_test_process::TestProcess::current(test)
            .unwrap()
            .name()
    );
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).unwrap();
    for line in stdout.lines().filter(|line| !line.is_empty()) {
        assert!(
            line == "running 1 test"
                || line == success
                || line.starts_with("test result: ok. 1 passed; 0 failed;"),
            "unexpected child output: {line}"
        );
    }
}
/// Cancel a live discovery probe and observe its owned socket closing.
#[cfg(unix)]
async fn abort_discovery() {
    use tokio::io::AsyncReadExt;

    let probes = std::env::var("MAESTRO_FIND_PROBE").unwrap();
    let listener =
        tokio::net::UnixListener::bind(std::path::Path::new(&probes).join("probe.sock")).unwrap();
    let signal = maestro_models::Cancellation::new();
    let empty = read_support::Scratch::new().unwrap();
    let tool = maestro_tools::create_find_tool(
        empty.text(),
        FindToolOptions::default(),
        Arc::new(ToolsManager::new(empty.text(), None)),
    );
    let mut work = Box::pin((tool.execute)(
        "call".into(),
        serde_json::json!({"pattern":"*"}),
        Some(signal.clone()),
        None,
    ));
    let (mut socket, _) = tokio::select! {r=&mut work=>panic!("discovery settled: {}",r.is_ok()),s=listener.accept()=>s.unwrap()};
    let mut ready = [0; 5];
    socket.read_exact(&mut ready).await.unwrap();
    assert_eq!(&ready, b"ready");
    signal.abort();
    assert_eq!(work.await.err().unwrap().message, "Operation aborted");
    assert_eq!(socket.read(&mut [0]).await.unwrap(), 0);
}
/// Observe unavailable acquisition after pre-abort proves no native work.
#[cfg(unix)]
async fn unavailable_and_pre_abort(root: &read_support::Scratch) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let calls = Arc::new(AtomicUsize::new(0));
    let manager = find_support::manager(root.text(), &calls);
    let tool = maestro_tools::create_find_tool("/work".into(), FindToolOptions::default(), manager);
    let signal = maestro_models::Cancellation::new();
    signal.abort();
    assert_eq!(
        (tool.execute)(
            "call".into(),
            serde_json::json!({"pattern":"*"}),
            Some(signal),
            None
        )
        .await
        .err()
        .unwrap()
        .message,
        "Operation aborted"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let updates = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&updates);
    let update: maestro_agent::AgentToolUpdateCallback = Arc::new(move |_| {
        count.fetch_add(1, Ordering::SeqCst);
    });
    let unavailable =
        find_support::cases("find_native_failures_and_partial_output_follow_exit_rules")
            .into_iter()
            .find(|c| c.id == "unavailable")
            .unwrap();
    assert_eq!(unavailable.op, "native-controlled");
    assert_eq!(unavailable.input, serde_json::json!({"unavailable":true}));
    assert_eq!(
        (tool.execute)(
            "call".into(),
            serde_json::json!({"pattern":"*"}),
            None,
            Some(update)
        )
        .await
        .err()
        .unwrap()
        .message,
        unavailable.expected["error"].as_str().unwrap()
    );
    assert_eq!(
        serde_json::json!(updates.load(Ordering::SeqCst)),
        unavailable.expected["updates"]
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
/// Cancel after the supplied acquisition transport registers its started witness.
#[cfg(unix)]
async fn abort_download(root: &read_support::Scratch) {
    use futures_util::FutureExt;
    let started = maestro_models::Cancellation::new();
    let witness = started.clone();
    let fetch: maestro_models::Fetch = Arc::new(move |_| {
        let started = witness.clone();
        Box::pin(async move {
            started.abort();
            std::future::pending().await
        })
    });
    let tool = maestro_tools::create_find_tool(
        root.text(),
        FindToolOptions::default(),
        Arc::new(ToolsManager::new(root.text(), Some(fetch))),
    );
    let signal = maestro_models::Cancellation::new();
    let mut work = Box::pin((tool.execute)(
        "call".into(),
        serde_json::json!({"pattern":"*"}),
        Some(signal.clone()),
        None,
    ));
    tokio::select! {r=&mut work=>panic!("acquisition settled: {}",r.is_ok()),()=started.cancelled()=>{}}
    signal.abort();
    assert_eq!(
        work.as_mut().now_or_never().unwrap().err().unwrap().message,
        "Operation aborted"
    );
    drop(work);
}
