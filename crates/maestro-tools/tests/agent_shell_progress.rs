//! Synchronous progress and per-call throttling through supplied operations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::agent_shell_support;
use crate::process_support;
use crate::runtime::{self, run_paused};
use agent_shell_support::{Immediate, cases, result_value};
use maestro_tools::{BashExecResult, BashToolInput, BashToolOptions, create_bash_tool_definition};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[test]
fn bash_progress_starts_synchronously_before_operations() {
    let phases = Arc::new(Mutex::new(Vec::new()));
    let hook_phases = phases.clone();
    let hook = Arc::new(move |context| {
        hook_phases.lock().unwrap().push("hook");
        Ok(context)
    });
    let exec_phases = phases.clone();
    let ops = Immediate(Arc::new(move |_, _, _| {
        assert_eq!(*exec_phases.lock().unwrap(), ["hook", "update"]);
        exec_phases.lock().unwrap().push("exec");
        Ok(BashExecResult {
            exit_code: Some(0.0),
        })
    }));
    let definition = create_bash_tool_definition::<()>(
        "/work".into(),
        "/bin".into(),
        BashToolOptions {
            operations: Some(Arc::new(ops)),
            spawn_hook: Some(hook),
            ..BashToolOptions::default()
        },
    );
    let updates = Arc::new(Mutex::new(Vec::new()));
    let update_values = updates.clone();
    let update_phases = phases.clone();
    let update = Arc::new(move |value| {
        update_values.lock().unwrap().push(result_value(value));
        update_phases.lock().unwrap().push("update");
    });
    let future = (definition.execute)(
        "call".into(),
        BashToolInput {
            command: "literal".into(),
            timeout: None,
        },
        None,
        Some(update),
        None,
    );
    assert_eq!(*phases.lock().unwrap(), ["hook", "update"]);
    assert_eq!(*updates.lock().unwrap(), [json!({"content":[]})]);
    assert_eq!(
        result_value(run_paused(future).unwrap()),
        json!({"content":[{"type":"text","text":"(no output)"}]})
    );
    assert_eq!(*phases.lock().unwrap(), ["hook", "update", "exec"]);
    assert_eq!(updates.lock().unwrap().len(), 1);
}

#[test]
fn bash_progress_emits_first_chunk_without_waiting() {
    cases(&["immediate_first"]);
    let updates = Arc::new(Mutex::new(Vec::new()));
    let seen = updates.clone();
    let ops = Immediate(Arc::new(move |_, _, options| {
        (options.on_data)(b"");
        assert_eq!(
            *seen.lock().unwrap(),
            [
                json!({"content":[]}),
                json!({"content":[{"type":"text","text":""}],"details":{}})
            ]
        );
        Ok(BashExecResult {
            exit_code: Some(0.0),
        })
    }));
    let definition = create_bash_tool_definition::<()>(
        "/work".into(),
        "/bin".into(),
        BashToolOptions {
            operations: Some(Arc::new(ops)),
            ..BashToolOptions::default()
        },
    );
    let seen = updates;
    let future = (definition.execute)(
        "call".into(),
        BashToolInput {
            command: "literal".into(),
            timeout: None,
        },
        None,
        Some(Arc::new(move |value| {
            seen.lock().unwrap().push(result_value(value));
        })),
        None,
    );
    run_paused(future).unwrap();
}

#[test]
fn bash_progress_coalesces_until_selected_deadline() {
    cases(&[
        "coalesced_final",
        "timed_update",
        "new_period_after_idle",
        "chatty_5000",
    ]);
    runtime::run(async {
        let release = Arc::new(tokio::sync::Notify::new());
        let definition = create_bash_tool_definition::<()>(
            "/work".into(),
            "/bin".into(),
            BashToolOptions {
                operations: Some(Arc::new(Held {
                    release: release.clone(),
                    outcome: 0,
                })),
                ..BashToolOptions::default()
            },
        );
        let delivered = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let seen = delivered.clone();
        let update = Arc::new(move |value| {
            let value = result_value(value);
            if value
                .pointer("/content/0/text")
                .and_then(serde_json::Value::as_str)
                == Some("AB")
            {
                seen.store(true, std::sync::atomic::Ordering::SeqCst);
                release.notify_one();
            }
        });
        let result = (definition.execute)(
            "call".into(),
            BashToolInput {
                command: "literal".into(),
                timeout: None,
            },
            None,
            Some(update),
            None,
        )
        .await
        .unwrap();
        assert!(
            delivered.load(std::sync::atomic::Ordering::SeqCst),
            "real timer delivered queued output before operation settlement"
        );
        assert_eq!(
            result_value(result),
            json!({"content":[{"type":"text","text":"AB"}]})
        );
    });
}

#[test]
fn bash_progress_flushes_dirty_output_on_every_settlement() {
    cases(&[
        "dirty_success",
        "dirty_missing_status",
        "dirty_nonzero",
        "dirty_abort",
        "dirty_timeout",
        "dirty_failure",
    ]);
}

#[test]
fn bash_progress_finishes_decoder_without_synthetic_clean_update() {
    cases(&["utf8_incomplete_clean", "utf8_incomplete_dirty"]);
}

#[test]
fn bash_progress_without_observer_needs_no_timer() {
    cases(&["no_observer"]);
    let ops = Immediate(Arc::new(|_, _, options| {
        (options.on_data)(&vec![b'A'; 51201]);
        Ok(BashExecResult {
            exit_code: Some(0.0),
        })
    }));
    let definition = create_bash_tool_definition::<()>(
        "/work".into(),
        "/bin".into(),
        BashToolOptions {
            operations: Some(Arc::new(ops)),
            ..BashToolOptions::default()
        },
    );
    let mut future = (definition.execute)(
        "call".into(),
        BashToolInput {
            command: "literal".into(),
            timeout: None,
        },
        None,
        None,
        None,
    );
    let poll = future
        .as_mut()
        .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()));
    let std::task::Poll::Ready(Ok(result)) = poll else {
        panic!("immediate supplied operation must finish without a runtime");
    };
    let details = result.details.unwrap();
    let path = details.full_output_path.unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), vec![b'A'; 51201]);
    std::fs::remove_file(path).unwrap();
    assert!(details.truncation.unwrap().last_line_partial);
}

#[test]
fn bash_notice_names_the_oversized_line_that_ends_with_a_newline() {
    let ops = Immediate(Arc::new(|_, _, options| {
        (options.on_data)(&[vec![b'A'; 60000], b"\n".to_vec()].concat());
        Ok(BashExecResult {
            exit_code: Some(0.0),
        })
    }));
    let definition = create_bash_tool_definition::<()>(
        "/work".into(),
        "/bin".into(),
        BashToolOptions {
            operations: Some(Arc::new(ops)),
            ..BashToolOptions::default()
        },
    );
    let mut future = (definition.execute)(
        "call".into(),
        BashToolInput {
            command: "literal".into(),
            timeout: None,
        },
        None,
        None,
        None,
    );
    let poll = future
        .as_mut()
        .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()));
    let std::task::Poll::Ready(Ok(result)) = poll else {
        panic!("immediate supplied operation must finish without a runtime");
    };
    let details = result.details.unwrap();
    let path = details.full_output_path.unwrap();
    let text = json!(result.content)[0]["text"]
        .as_str()
        .unwrap()
        .to_owned();
    std::fs::remove_file(&path).unwrap();
    let kept = "A".repeat(51200);
    assert_eq!(
        text,
        format!("{kept}\n\n[Showing last 50.0KB of line 1 (line is 58.6KB). Full output: {path}]")
    );
    assert_eq!(details.truncation.unwrap().total_bytes, 60001);
}

#[test]
fn bash_notice_names_the_oversized_line_after_an_earlier_newline_in_one_chunk() {
    let chunk = [b"head\n".to_vec(), vec![b'A'; 61491], b"\n".to_vec()].concat();
    let ops = Immediate(Arc::new(move |_, _, options| {
        (options.on_data)(&chunk);
        Ok(BashExecResult {
            exit_code: Some(0.0),
        })
    }));
    let definition = create_bash_tool_definition::<()>(
        "/work".into(),
        "/bin".into(),
        BashToolOptions {
            operations: Some(Arc::new(ops)),
            ..BashToolOptions::default()
        },
    );
    let mut future = (definition.execute)(
        "call".into(),
        BashToolInput {
            command: "literal".into(),
            timeout: None,
        },
        None,
        None,
        None,
    );
    let poll = future
        .as_mut()
        .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()));
    let std::task::Poll::Ready(Ok(result)) = poll else {
        panic!("immediate supplied operation must finish without a runtime");
    };
    let details = result.details.unwrap();
    let path = details.full_output_path.unwrap();
    let text = json!(result.content)[0]["text"]
        .as_str()
        .unwrap()
        .to_owned();
    std::fs::remove_file(&path).unwrap();
    assert!(
        text.ends_with(&format!(
            "[Showing last 50.0KB of line 2 (line is 60.0KB). Full output: {path}]"
        )),
        "{text}"
    );
}

/// Holds the original operation pending after two chunks, then settles as selected.
struct Held {
    release: Arc<tokio::sync::Notify>,
    outcome: usize,
}
impl maestro_tools::BashOperations for Held {
    fn exec<'a>(
        &'a self,
        _: &'a str,
        _: &'a str,
        options: maestro_tools::BashExecOptions<'a>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<BashExecResult, maestro_tools::BashError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            (options.on_data)(b"A");
            (options.on_data)(b"B");
            self.release.notified().await;
            match self.outcome {
                0 => Ok(BashExecResult {
                    exit_code: Some(0.0),
                }),
                1 => Ok(BashExecResult {
                    exit_code: Some(7.0),
                }),
                2 => Ok(BashExecResult { exit_code: None }),
                3 => Err(maestro_tools::BashError::Aborted),
                4 => Err(maestro_tools::BashError::Timeout(5.0)),
                _ => Err(std::io::Error::other("offline").into()),
            }
        })
    }
}

#[test]
fn bash_progress_settlement_releases_observers() {
    run_paused(async {
        for outcome in 0..6 {
            let release = Arc::new(tokio::sync::Notify::new());
            let definition = create_bash_tool_definition::<()>(
                "/work".into(),
                "/bin".into(),
                BashToolOptions {
                    operations: Some(Arc::new(Held {
                        release: release.clone(),
                        outcome,
                    })),
                    ..BashToolOptions::default()
                },
            );
            let sentinel = Arc::new(());
            let captured = sentinel.clone();
            let updates = Arc::new(Mutex::new(Vec::new()));
            let seen = updates.clone();
            let observer = Arc::new(move |value| {
                let _ = &captured;
                seen.lock().unwrap().push(result_value(value));
            });
            let signal = maestro_models::Cancellation::new();
            let mut execution = Box::pin((definition.execute)(
                "call".into(),
                BashToolInput {
                    command: "literal".into(),
                    timeout: None,
                },
                Some(signal.clone()),
                Some(observer),
                None,
            ));
            let forward = process_support::forward();
            assert!(
                std::future::poll_fn(|cx| std::task::Poll::Ready(process_support::poll_through(
                    &forward,
                    &mut execution,
                    cx
                )))
                .await
                .is_pending()
            );
            assert_eq!(Arc::strong_count(&sentinel), 2);
            release.notify_one();
            let result = process_support::complete(&forward, execution).await;
            assert_eq!(result.is_ok(), matches!(outcome, 0 | 2));
            assert_eq!(Arc::strong_count(&forward), 1);
            assert_eq!(Arc::strong_count(&sentinel), 1);
            let before = updates.lock().unwrap().len();
            signal.abort();
            tokio::time::advance(std::time::Duration::from_secs(1)).await;
            assert_eq!(updates.lock().unwrap().len(), before);
            assert_eq!(definition.definition.name, "bash");
        }
    });
}
