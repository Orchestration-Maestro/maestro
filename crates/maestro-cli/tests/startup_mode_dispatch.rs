//! Final startup dispatch over real services and supplied presentation contracts.
use crate::startup_runtime::run;
use crate::support::mode_dispatch::{
    BENCHMARK_TIMINGS, Harness, PRIOR_TIMINGS, request, scoped, timing,
};
use maestro_cli::startup::main::bootstrap::AppMode;

#[test]
fn benchmark_flags_reject_only_selected_noninteractive_modes() {
    run(
        "benchmark_flags_reject_only_selected_noninteractive_modes",
        |local| async move {
            let cases: Vec<FlagCase> =
                serde_json::from_str(include_str!("fixtures/dispatch_flags.json")).unwrap();
            assert_eq!(cases.len(), 17);
            for (case, mode) in cases.iter().flat_map(|case| {
                [AppMode::Print, AppMode::Json, AppMode::Rpc].map(|mode| (case, mode))
            }) {
                let mut h = Harness::new(local.clone(), false).await;
                h.styles = true;
                let mut input = request(mode);
                input.startup_benchmark.clone_from(&case.input);
                assert_eq!(h.dispatch(input).await.unwrap(), i32::from(case.expected));
                if case.expected {
                    assert_eq!(
                        h.recorder.events(),
                        [
                            "stderr:\u{1b}[31mError: MAESTRO_STARTUP_BENCHMARK only supports interactive mode\u{1b}[39m\n"
                        ]
                    );
                } else {
                    assert_eq!(
                        h.recorder.events(),
                        if mode == AppMode::Rpc {
                            vec!["rpc"]
                        } else {
                            Vec::new()
                        }
                    );
                }
            }
        },
    );
}

/// One exact benchmark-flag decision.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FlagCase {
    /// Native environment string, with absence distinct from empty.
    input: Option<String>,
    /// Whether this flag requests the benchmark.
    expected: bool,
}

/// A captured model's identifier.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeId {
    /// Authored identifier bytes.
    id: String,
}
/// A captured model and optional typed effort.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeMember {
    /// Original descriptor identity label.
    model: ScopeId,
    /// The admitted effort spelling.
    #[serde(rename = "thinkingLevel")]
    thinking_level: Option<String>,
}
/// One emitted styled banner from the recorded expression.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeCase {
    /// Members in startup snapshot order.
    input: Vec<ScopeMember>,
    /// Exact nested styling before the added newline.
    expected: String,
}

#[test]
fn scope_banner_keeps_snapshot_labels_and_nested_styles() {
    run(
        "scope_banner_keeps_snapshot_labels_and_nested_styles",
        |local| async move {
            let mut h = Harness::new(local, false).await;
            h.styles = true;
            let cases: Vec<ScopeCase> =
                serde_json::from_str(include_str!("fixtures/dispatch_scope.json")).unwrap();
            assert_eq!(cases.len(), 5);
            for case in cases {
                let mut input = request(AppMode::Interactive);
                input.scoped_models = case
                    .input
                    .iter()
                    .map(|m| scoped(&h, &m.model.id, m.thinking_level.as_deref()))
                    .collect();
                assert_eq!(h.runtime.session().scoped_models().len(), 0);
                h.recorder.log.borrow_mut().clear();
                assert_eq!(h.dispatch(input).await.unwrap(), 0);
                assert_eq!(h.recorder.bytes("stdout"), format!("{}\n", case.expected));
                assert_eq!(
                    h.recorder.events(),
                    [
                        "factory".into(),
                        format!("stdout:{}\n", case.expected),
                        "run".into()
                    ]
                );
            }
        },
    );
}

#[test]
fn scope_visibility_varies_quiet_and_verbose_independently() {
    run(
        "scope_visibility_varies_quiet_and_verbose_independently",
        |local| async move {
            for (quiet, verbose, populated) in [
                (false, false, false),
                (false, false, true),
                (false, true, false),
                (false, true, true),
                (true, false, false),
                (true, false, true),
                (true, true, false),
                (true, true, true),
            ] {
                let mut h = Harness::new(local.clone(), quiet).await;
                let mut input = request(AppMode::Interactive);
                input.interactive_options.verbose = verbose;
                if populated {
                    input.scoped_models = vec![scoped(&h, "captured", None)];
                }
                h.dispatch(input).await.unwrap();
                assert_eq!(
                    h.recorder.bytes("stdout"),
                    if populated && (verbose || !quiet) {
                        "Model scope: captured (Ctrl+P to cycle)\n"
                    } else {
                        ""
                    }
                );
            }
        },
    );
}

#[test]
fn scope_hint_names_the_configured_cycle_key_and_is_absent_when_unbound() {
    run(
        "scope_hint_names_the_configured_cycle_key_and_is_absent_when_unbound",
        |local| async move {
            for (binding, line) in [
                ("\"ctrl+o\"", "Model scope: captured (Ctrl+O to cycle)\n"),
                (
                    "[\"alt+x\", \"ctrl+y\"]",
                    "Model scope: captured (Alt+X/Ctrl+Y to cycle)\n",
                ),
                ("[]", "Model scope: captured\n"),
            ] {
                let scratch = maestro_test_directory::Scratch::new().unwrap();
                std::fs::write(
                    scratch.path().join("keybindings.json"),
                    format!("{{\"app.model.cycleForward\": {binding}}}"),
                )
                .unwrap();
                let h = Harness::new(local.clone(), false).await;
                *h.recorder.agent_dir.borrow_mut() =
                    Some(scratch.path().to_str().unwrap().to_owned());
                let mut h = h;
                let mut input = request(AppMode::Interactive);
                input.scoped_models = vec![scoped(&h, "captured", None)];
                h.dispatch(input).await.unwrap();
                assert_eq!(h.recorder.bytes("stdout"), line, "{binding}");
            }
        },
    );
}

#[test]
fn interactive_dispatch_times_then_runs_without_cleanup() {
    run(
        "interactive_dispatch_times_then_runs_without_cleanup",
        |local| async move {
            let mut h = Harness::new(local, false).await;
            for flag in [None, Some("0"), Some("1")] {
                h.timings = maestro_app::configuration::timings::Timings::new(flag, || 0.0);
                h.dispatch(request(AppMode::Interactive)).await.unwrap();
                assert_eq!(h.recorder.events(), ["factory", "run"]);
                h.recorder.log.borrow_mut().clear();
            }
            timing(&mut h);
            h.watch();
            h.recorder.log.borrow_mut().clear();
            h.dispatch(request(AppMode::Interactive)).await.unwrap();
            assert_eq!(
                h.recorder.events(),
                [
                    "factory".into(),
                    format!("stderr:{PRIOR_TIMINGS}"),
                    "run".into()
                ]
            );
            h.themes.stop_theme_watcher();
            assert_eq!(h.recorder.events().last().unwrap(), "watch-stop");
        },
    );
}

#[test]
fn benchmark_dispatch_stops_then_flushes_selected_owners() {
    run(
        "benchmark_dispatch_stops_then_flushes_selected_owners",
        |local| async move {
            let mut h = Harness::new(local, false).await;
            timing(&mut h);
            h.watch();
            let other = crate::support::presenters::theme(std::path::Path::new(&h.runtime.cwd()));
            let other_log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            other
                .init_theme(
                    Some("custom"),
                    Some(std::rc::Rc::new(
                        crate::support::presenters::RecordingWatch {
                            log: other_log.clone(),
                        },
                    )),
                )
                .unwrap();
            h.recorder.log.borrow_mut().clear();
            let mut input = request(AppMode::Interactive);
            input.startup_benchmark = Some("yes".into());
            assert_eq!(h.dispatch(input).await.unwrap(), 0);
            assert_eq!(
                h.recorder.events(),
                [
                    "factory".into(),
                    "init".into(),
                    format!("stderr:{BENCHMARK_TIMINGS}"),
                    "stop".into(),
                    "watch-stop".into(),
                    "stdout.flush".into(),
                    "stderr.flush".into()
                ]
            );
            assert_eq!(*other_log.borrow(), ["watch"]);
            other.stop_theme_watcher();
            assert_eq!(*other_log.borrow(), ["watch", "watch-stop"]);
        },
    );
}

#[test]
fn rpc_dispatch_receives_runtime_after_timings() {
    run(
        "rpc_dispatch_receives_runtime_after_timings",
        |local| async move {
            let mut h = Harness::new(local, false).await;
            timing(&mut h);
            let mut input = request(AppMode::Rpc);
            input.interactive_options.initial_message = Some("must not be sent".into());
            assert_eq!(h.dispatch(input).await.unwrap(), 0);
            assert_eq!(
                h.recorder.events(),
                [format!("stderr:{PRIOR_TIMINGS}"), "rpc".into()]
            );
            assert!(std::rc::Rc::ptr_eq(
                h.recorder.runtime.borrow().as_ref().unwrap(),
                &h.runtime
            ));
            assert!(h.recorder.options.borrow().is_none());
        },
    );
}

#[test]
fn interactive_dispatch_moves_every_startup_operand() {
    run(
        "interactive_dispatch_moves_every_startup_operand",
        |local| async move {
            use std::sync::{Arc, Mutex};
            let mut h = Harness::new(local, false).await;
            let observed = Arc::new(Mutex::new(Vec::new()));
            let seen = Arc::clone(&observed);
            let fetch: maestro_app::Fetch = Arc::new(move |request| {
                seen.lock()
                    .unwrap()
                    .push((request.method, request.url, request.body));
                Box::pin(async { Err(maestro_app::FetchError::Aborted) })
            });
            for images in [
                None,
                Some(Vec::new()),
                Some(vec![image("first"), image("second")]),
            ] {
                let mut input = request(AppMode::Interactive);
                input.interactive_options =
                    maestro_app::presentation_data::mode_dispatch::InteractiveModeOptions {
                        verbose: true,
                        migrated_providers: vec!["b".into(), "a".into(), "b".into()],
                        model_fallback_message: Some(String::new()),
                        initial_message: Some("first prompt".into()),
                        initial_images: images.clone(),
                        initial_messages: vec!["rest two".into(), "rest one".into()],
                        service_origin: Some("https://service.example.test/".into()),
                        fetch: Some(Arc::clone(&fetch)),
                    };
                h.recorder.log.borrow_mut().clear();
                h.dispatch(input).await.unwrap();
                assert_startup_operands(&h, images, &fetch).await;
            }
            assert_eq!(
                *observed.lock().unwrap(),
                vec![
                    (
                        "POST".into(),
                        "https://service.example.test/probe".into(),
                        b"payload".to_vec()
                    );
                    3
                ]
            );
        },
    );
}

/// Exact moved operands, including a real call through the supplied transport.
async fn assert_startup_operands(
    h: &Harness,
    images: Option<Vec<maestro_app::ImageContent>>,
    fetch: &maestro_app::Fetch,
) {
    use std::{rc::Rc, sync::Arc};
    assert_eq!(h.recorder.events(), ["factory", "run"]);
    assert!(Rc::ptr_eq(
        h.recorder.runtime.borrow().as_ref().unwrap(),
        &h.runtime
    ));
    let options = h.recorder.options.borrow_mut().take().unwrap();
    assert!(options.verbose);
    assert_eq!(options.migrated_providers, ["b", "a", "b"]);
    assert_eq!(options.model_fallback_message.as_deref(), Some(""));
    assert_eq!(options.initial_message.as_deref(), Some("first prompt"));
    assert_eq!(options.initial_images, images);
    assert_eq!(options.initial_messages, ["rest two", "rest one"]);
    assert_eq!(
        options.service_origin.as_deref(),
        Some("https://service.example.test/")
    );
    let received = options.fetch.unwrap();
    assert!(Arc::ptr_eq(&received, fetch));
    assert!(matches!(
        received(maestro_app::HttpRequest {
            method: "POST".into(),
            url: "https://service.example.test/probe".into(),
            headers: indexmap::IndexMap::default(),
            body: b"payload".to_vec(),
            signal: None
        })
        .await,
        Err(maestro_app::FetchError::Aborted)
    ));
}

/// Distinguishable ordered image operands.
fn image(data: &str) -> maestro_app::ImageContent {
    maestro_app::ImageContent {
        data: data.into(),
        mime_type: "image/png".into(),
    }
}

#[test]
fn dispatch_failures_keep_their_original_phase() {
    run(
        "dispatch_failures_keep_their_original_phase",
        |local| async move {
            use crate::support::mode_dispatch::Failure;
            for phase in [
                "factory",
                "init",
                "run",
                "rpc",
                "stop",
                "stdout.write",
                "stderr.write",
                "stdout.flush",
                "stderr.flush",
            ] {
                let mut h = Harness::new(local.clone(), false).await;
                timing(&mut h);
                h.watch();
                h.recorder.log.borrow_mut().clear();
                h.recorder.fail.set(Some(phase));
                let mut input = request(if phase == "rpc" {
                    AppMode::Rpc
                } else {
                    AppMode::Interactive
                });
                if !matches!(phase, "run" | "rpc") {
                    input.startup_benchmark = Some("1".into());
                }
                if phase == "stdout.write" {
                    input.scoped_models = vec![scoped(&h, "scope", None)];
                }
                let error = h.dispatch(input).await.unwrap_err();
                let original = error
                    .downcast_ref::<Failure>()
                    .or_else(|| {
                        error
                            .downcast_ref::<std::io::Error>()
                            .and_then(|e| e.get_ref())
                            .and_then(|e| e.downcast_ref::<Failure>())
                    })
                    .unwrap();
                assert_eq!(original.phase, phase);
                assert!(std::sync::Arc::ptr_eq(
                    &original.identity,
                    &h.recorder.identity
                ));
                assert_eq!(h.recorder.events(), failure_events(phase), "{phase}");
                h.themes.stop_theme_watcher();
                assert_eq!(
                    h.recorder
                        .events()
                        .iter()
                        .filter(|e| *e == "watch-stop")
                        .count(),
                    1
                );
            }
        },
    );
}

/// Every completed prefix before the selected failure, retaining writer bytes.
fn failure_events(phase: &str) -> Vec<String> {
    let timing = format!("stderr:{BENCHMARK_TIMINGS}");
    let prior = format!("stderr:{PRIOR_TIMINGS}");
    let expected: Vec<&str> = match phase {
        "factory" => vec!["factory"],
        "init" => vec!["factory", "init"],
        "run" => vec!["factory", &prior, "run"],
        "rpc" => vec![&prior, "rpc"],
        "stop" => vec!["factory", "init", &timing, "stop"],
        "stdout.write" => vec!["factory", "stdout.write"],
        "stderr.write" => vec!["factory", "init", "stderr.write"],
        "stdout.flush" => vec![
            "factory",
            "init",
            &timing,
            "stop",
            "watch-stop",
            "stdout.flush",
        ],
        "stderr.flush" => vec![
            "factory",
            "init",
            &timing,
            "stop",
            "watch-stop",
            "stdout.flush",
            "stderr.flush",
        ],
        _ => unreachable!(),
    };
    expected.into_iter().map(str::to_owned).collect()
}

#[test]
fn print_dispatch_uses_real_modes_and_returns_status() {
    let name = "print_dispatch_uses_real_modes_and_returns_status";
    if maestro_test_process::selection().is_child() {
        run(name, print_child);
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    for case in ["text-ok", "text-error", "json-ok", "json-error"] {
        let output = crate::support::missing_cwd::command(name, case, root.path())
            .quiet()
            .nocapture()
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{case}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = std::str::from_utf8(&output.stdout).unwrap();
        let body = stdout
            .strip_prefix("\nrunning 1 test\n")
            .unwrap()
            .split("\n.\n")
            .next()
            .unwrap();
        assert_print_output(case, body, &output.stderr);
    }
}

/// Native print bytes observed outside the isolated runtime.
fn assert_print_output(case: &str, body: &str, stderr: &[u8]) {
    if case == "text-ok" {
        assert_eq!(body, "answer");
    }
    if case == "json-ok" {
        let records: Vec<serde_json::Value> = body
            .lines()
            .take_while(|line| line.starts_with('{'))
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(records[0]["type"], "session");
        let starts: Vec<_> = records
            .iter()
            .filter(|event| event["type"] == "message_start" && event["message"]["role"] == "user")
            .map(|event| event["message"]["content"][0]["text"].as_str().unwrap())
            .collect();
        assert_eq!(starts, ["first prompt", "rest two", "rest one"]);
        assert_eq!(
            records
                .iter()
                .filter(|event| event["type"] == "agent_end")
                .count(),
            3
        );
    }
    if case.ends_with("error") {
        let diagnostics = String::from_utf8_lossy(stderr);
        assert!(
            diagnostics.contains("No API key found for startup."),
            "{case}: {diagnostics}"
        );
    }
}

/// Run the real print owner, keeping its provider and cleanup observations.
async fn print_child(local: std::rc::Rc<tokio::task::LocalSet>) {
    use maestro_test_process::selection;
    let mut h = Harness::new(local, false).await;
    timing(&mut h);
    h.watch();
    h.recorder.log.borrow_mut().clear();
    let prompts = crate::support::mode_dispatch::script_print(&h);
    let fails = selection().runs_case("text-error") || selection().runs_case("json-error");
    if fails {
        h.runtime
            .services()
            .model_registry
            .unregister_provider("startup")
            .unwrap();
    }
    let recorder = std::rc::Rc::clone(&h.recorder);
    h.runtime
        .set_before_session_invalidate(Some(std::rc::Rc::new(move || {
            assert!(maestro_app::output_guard::is_stdout_taken_over());
            recorder.call("dispose").unwrap();
            Ok(())
        })));
    maestro_app::output_guard::take_over_stdout();
    let input = print_request();
    assert_eq!(h.dispatch(input).await.unwrap(), i32::from(fails));
    assert!(!maestro_app::output_guard::is_stdout_taken_over());
    assert!(h.recorder.runtime.borrow().is_none());
    assert!(h.recorder.options.borrow().is_none());
    assert_eq!(h.recorder.events(), print_events(fails));
    if fails {
        assert!(prompts.borrow().is_empty());
    } else {
        assert_print_prompts(&prompts.borrow());
    }
}

/// Prepared ordered prompts for either print representation.
fn print_request() -> maestro_cli::startup::main::dispatch::ModeDispatch {
    use maestro_test_process::selection;
    let mut input = request(
        if selection().runs_case("json-ok") || selection().runs_case("json-error") {
            AppMode::Json
        } else {
            AppMode::Print
        },
    );
    input.interactive_options.initial_message = Some("first prompt".into());
    input.interactive_options.initial_messages = vec!["rest two".into(), "rest one".into()];
    input.interactive_options.initial_images = Some(vec![image("first"), image("second")]);
    input
}

/// Completed provider and cleanup phases of an actual print run.
fn print_events(fails: bool) -> Vec<String> {
    let expected: Vec<String> = if fails {
        vec![
            format!("stderr:{PRIOR_TIMINGS}"),
            "dispose".into(),
            "watch-stop".into(),
        ]
    } else {
        vec![
            format!("stderr:{PRIOR_TIMINGS}"),
            "provider".into(),
            "provider".into(),
            "provider".into(),
            "dispose".into(),
            "watch-stop".into(),
        ]
    };
    expected
}

/// Provider-side observations of each prompt and the first images.
fn assert_print_prompts(prompts: &[serde_json::Value]) {
    assert_eq!(prompts.len(), 3);
    let content: Vec<_> = prompts
        .iter()
        .map(|message| message["content"].clone())
        .collect();
    assert_eq!(
        content,
        [
            serde_json::json!([
                {"type":"text","text":"first prompt"},
                {"type":"image","data":"first","mimeType":"image/png"},
                {"type":"image","data":"second","mimeType":"image/png"}
            ]),
            serde_json::json!([{"type":"text","text":"rest two"}]),
            serde_json::json!([{"type":"text","text":"rest one"}])
        ]
    );
}

#[test]
fn print_failure_does_not_restore_or_close_outer_owners() {
    use maestro_test_process::{run_alone, selection};
    use std::io::{Read, Write};
    let name = "print_failure_does_not_restore_or_close_outer_owners";
    if selection().is_child() {
        run(name, |local| async move {
            let mut h = Harness::new(local, false).await;
            h.watch();
            h.recorder.log.borrow_mut().clear();
            let mut ready = [0];
            std::io::stdin().read_exact(&mut ready).unwrap();
            assert_eq!(ready, [1]);
            maestro_app::output_guard::take_over_stdout();
            std::io::stdout().write_all(b"held").unwrap();
            let error = h.dispatch(request(AppMode::Json)).await.unwrap_err();
            assert!(
                error
                    .downcast_ref::<maestro_app::DiagnosticErrorInfo>()
                    .is_some()
            );
            assert!(maestro_app::output_guard::is_stdout_taken_over());
            assert!(h.recorder.events().is_empty());
            h.themes.stop_theme_watcher();
            assert_eq!(h.recorder.events(), ["watch-stop"]);
            eprintln!("PRINT-FAILURE-OBSERVED");
            std::process::exit(0);
        });
        return;
    }
    if run_alone(name).unwrap() {
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    let mut child = crate::support::missing_cwd::command(name, "broken-pipe", root.path())
        .quiet()
        .nocapture()
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    child.stdin.take().unwrap().write_all(&[1]).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("PRINT-FAILURE-OBSERVED"));
}
