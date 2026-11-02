//! Native waits, child controls and bytes through the controlled author imports.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{
    bus_callbacks::captured_api,
    controlled::{Controlled, Flag},
    observed::gate,
};
use crate::{ExtensionResult, child_process::SpawnOptions, component_adapter::signal};
use std::{
    task::{Context, Waker},
    time::Duration,
};

#[test]
fn native_wait_uses_the_selected_duration() -> ExtensionResult<()> {
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    for duration in [Duration::ZERO, Duration::new(2, 123)] {
        super::block_on(io.sleep(duration, None))?;
    }
    assert!(
        host.observed()
            .transcript
            .contains(&"native-wait 0 0".into())
    );
    assert!(
        host.observed()
            .transcript
            .contains(&"native-wait 2 123".into())
    );
    Ok(())
}

#[test]
fn native_wait_rejects_preabort_before_arming() -> ExtensionResult<()> {
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    let flag = Flag::default();
    flag.abort();
    let signal = signal(&host, host.signal(&flag));
    assert_eq!(
        super::block_on(io.sleep(Duration::from_secs(9), Some(signal))),
        Err("Aborted".into())
    );
    assert!(
        !host
            .observed()
            .transcript
            .iter()
            .any(|line| line.starts_with("native-wait"))
    );
    assert!(host.observed().bus.listeners.is_empty());
    Ok(())
}

#[test]
fn native_wait_releases_its_signal_observer() -> ExtensionResult<()> {
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    for outcome in ["complete", "abort", "drop"] {
        let flag = Flag::default();
        let signal = super::signal_callbacks::authored(signal(&host, host.signal(&flag)));
        let (hold, _started, open) = gate();
        host.observe().hold_next_idle(hold);
        let mut wait = io.sleep(Duration::from_secs(9), Some(signal));
        assert!(
            wait.as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        assert_eq!(host.observed().bus.listeners.len(), 1);
        if outcome == "drop" {
            drop(wait);
        } else {
            if outcome == "abort" {
                assert_eq!(host.abort(&flag), [Ok(())]);
            }
            open.send(()).unwrap();
            let expected = if outcome == "abort" {
                Err("Aborted".into())
            } else {
                Ok(())
            };
            assert_eq!(super::block_on(wait), expected);
        }
        assert!(host.observed().bus.listeners.is_empty());
    }
    Ok(())
}

#[test]
fn native_child_streams_bytes_while_input_is_pending() -> ExtensionResult<()> {
    let host = Controlled::default();
    host.observe().native_io.stdout = vec![vec![0, 255, 226], vec![130, 172]];
    host.observe().native_io.stderr = vec![b"error\0".to_vec()];
    let io = captured_api(&host)?.native_io();
    let child = super::block_on(io.spawn(
        "literal program".into(),
        vec!["$x; Ω\n".into()],
        SpawnOptions::default(),
    ))?;
    let stdin = child.stdin().unwrap();
    let (hold, _started, open) = gate();
    host.observe().hold_next_idle(hold);
    let mut written = stdin.write(&[0, 255, 1]);
    assert!(
        written
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    assert_eq!(
        super::block_on(child.stdout().unwrap().read())?,
        Some(vec![0, 255, 226])
    );
    assert_eq!(
        super::block_on(child.stderr().unwrap().read())?,
        Some(b"error\0".to_vec())
    );
    open.send(()).unwrap();
    super::block_on(written)?;
    super::block_on(stdin.end())?;
    assert_eq!(
        super::block_on(child.stdout().unwrap().read())?,
        Some(vec![130, 172])
    );
    assert_eq!(super::block_on(child.stdout().unwrap().read())?, None);
    let observed = host.observed();
    let original = observed.native_io.child.as_ref().unwrap();
    assert_eq!(
        *original.stdin.as_ref().unwrap().bytes.lock().unwrap(),
        [0, 255, 1]
    );
    assert!(
        original
            .stdin
            .as_ref()
            .unwrap()
            .ended
            .load(std::sync::atomic::Ordering::SeqCst)
    );
    assert_eq!(observed.native_io.launches[0].0, "literal program");
    assert_eq!(observed.native_io.launches[0].1, ["$x; Ω\n"]);
    Ok(())
}

#[test]
fn native_wait_keeps_the_original_host_signal() -> ExtensionResult<()> {
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    let flag = Flag::default();
    let original = signal(&host, host.signal(&flag));
    let (hold, _started, open) = gate();
    host.observe().hold_next_idle(hold);
    let mut wait = io.sleep(Duration::from_secs(9), Some(original.clone()));
    assert!(
        wait.as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    assert!(host.observed().bus.listeners.is_empty());
    drop(original);
    assert_eq!(host.observed().shell.signal_drops.get(&flag.key()), None);
    flag.abort();
    open.send(()).unwrap();
    assert_eq!(super::block_on(wait), Err("Aborted".into()));
    assert_eq!(
        host.observed().shell.signal_drops.get(&flag.key()),
        Some(&1)
    );
    Ok(())
}

#[test]
fn native_io_adapters_keep_the_same_author_contract() -> ExtensionResult<()> {
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    let child = super::block_on(io.spawn(
        "literal".into(),
        Vec::new(),
        crate::child_process::SpawnOptions::default(),
    ))?;
    assert!(std::rc::Rc::ptr_eq(
        &child.stdin().unwrap(),
        &child.stdin().unwrap()
    ));
    assert!(std::rc::Rc::ptr_eq(
        &child.stdout().unwrap(),
        &child.stdout().unwrap()
    ));
    assert!(std::rc::Rc::ptr_eq(
        &child.stderr().unwrap(),
        &child.stderr().unwrap()
    ));
    let response = super::block_on(io.fetch(http_request(None)))?;
    assert!(std::rc::Rc::ptr_eq(
        &response.body().unwrap(),
        &response.body().unwrap()
    ));
    let watch = io.watch(
        "/identity Ω".into(),
        std::rc::Rc::new(|emitter| emitter.emit("identity", serde_json::json!("watch Ω"))),
    )?;
    let alias = watch.clone();
    drop(watch);
    host.notify_watch(0)?;
    assert!(
        host.observed()
            .bus
            .emissions
            .contains(&("identity".into(), serde_json::json!("watch Ω")))
    );
    alias.close()?;
    assert!(
        host.observed().native_watch.watches[0]
            .upgrade()
            .unwrap()
            .lock()
            .unwrap()
            .is_none()
    );
    Ok(())
}

#[test]
fn native_child_keeps_literal_arguments_and_cwd() -> ExtensionResult<()> {
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    let arguments = vec!["space value", "$HOME; `literal`", "line\nbreak", "Ω雪", ""];
    for cwd in [None, Some("/outside Ω/../literal path".to_owned())] {
        super::block_on(io.spawn(
            "literal program Ω".into(),
            arguments.iter().map(|arg| (*arg).to_owned()).collect(),
            SpawnOptions {
                cwd: cwd.clone(),
                ..SpawnOptions::default()
            },
        ))?;
        let observed = host.observed();
        let (command, args, options) = observed.native_io.launches.last().unwrap();
        assert_eq!(command, "literal program Ω");
        assert_eq!(args, &arguments);
        assert_eq!(options.cwd, cwd);
    }
    Ok(())
}

#[test]
fn native_child_keeps_environment_selection() -> ExtensionResult<()> {
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    for env in [
        None,
        Some(Vec::new()),
        Some(vec![
            ("UNCHANGED".into(), "inherited Ω".into()),
            ("OVERLAY".into(), "replacement\n".into()),
            ("EMPTY".into(), String::new()),
        ]),
    ] {
        super::block_on(io.spawn(
            "literal".into(),
            Vec::new(),
            SpawnOptions {
                env: env.clone(),
                ..SpawnOptions::default()
            },
        ))?;
        assert_eq!(
            host.observed().native_io.launches.last().unwrap().2.env,
            env
        );
    }
    Ok(())
}

#[test]
fn native_child_keeps_descriptor_selection() -> ExtensionResult<()> {
    use crate::child_process::{SpawnOptions, Stdio};
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    for stdin in [Stdio::Pipe, Stdio::Ignore, Stdio::Inherit] {
        for stdout in [Stdio::Pipe, Stdio::Ignore, Stdio::Inherit] {
            for stderr in [Stdio::Pipe, Stdio::Ignore, Stdio::Inherit] {
                let stdio = [stdin, stdout, stderr];
                let child = super::block_on(io.spawn(
                    "literal".into(),
                    Vec::new(),
                    SpawnOptions {
                        stdio,
                        ..SpawnOptions::default()
                    },
                ))?;
                assert_eq!(
                    [
                        child.stdin().is_some(),
                        child.stdout().is_some(),
                        child.stderr().is_some()
                    ],
                    stdio.map(|descriptor| descriptor == Stdio::Pipe)
                );
                assert_eq!(
                    host.observed().native_io.launches.last().unwrap().2.stdio,
                    stdio
                );
            }
        }
    }
    Ok(())
}

#[test]
fn native_child_writes_all_bytes_before_eof() -> ExtensionResult<()> {
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    let child = super::block_on(io.spawn("literal".into(), Vec::new(), SpawnOptions::default()))?;
    let input = child.stdin().unwrap();
    let bytes: Vec<u8> = (0..131_089)
        .map(|value| u8::try_from(value % 256).unwrap())
        .collect();
    super::block_on(input.write(&[]))?;
    super::block_on(input.write(&bytes))?;
    super::block_on(input.write(&[255, 0, 226, 130, 172]))?;
    super::block_on(input.end())?;
    let mut expected = bytes;
    expected.extend([255, 0, 226, 130, 172]);
    let observed = host.observed();
    let original = observed
        .native_io
        .child
        .as_ref()
        .unwrap()
        .stdin
        .as_ref()
        .unwrap();
    assert_eq!(*original.bytes.lock().unwrap(), expected);
    assert!(original.ended.load(std::sync::atomic::Ordering::SeqCst));
    drop(observed);
    assert!(super::block_on(input.write(&[1])).is_err());
    Ok(())
}

#[test]
fn native_child_completion_keeps_output_close_order() -> ExtensionResult<()> {
    let host = Controlled::default();
    host.observe().native_io.stdout = vec![b"early".to_vec(), b"late\0\xff".to_vec()];
    host.observe().native_io.stderr = vec![b"early-error".to_vec(), b"late-error".to_vec()];
    let io = captured_api(&host)?.native_io();
    let child = super::block_on(io.spawn("literal".into(), Vec::new(), SpawnOptions::default()))?;
    let stdout = child.stdout().unwrap();
    let stderr = child.stderr().unwrap();
    assert_eq!(super::block_on(stdout.read())?, Some(b"early".to_vec()));
    assert_eq!(
        super::block_on(stderr.read())?,
        Some(b"early-error".to_vec())
    );
    let (hold, _started, open) = gate();
    host.observe().hold_next_idle(hold);
    let mut wait = child.wait();
    assert!(
        wait.as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    open.send(()).unwrap();
    assert_eq!(super::block_on(wait)?.code, Some(7));
    assert_eq!(
        super::block_on(stdout.read())?,
        Some(b"late\0\xff".to_vec())
    );
    assert_eq!(
        super::block_on(stderr.read())?,
        Some(b"late-error".to_vec())
    );
    assert_eq!(super::block_on(stdout.read())?, None);
    assert_eq!(super::block_on(stderr.read())?, None);
    Ok(())
}

#[test]
fn native_child_keeps_exit_and_signal_distinct() -> ExtensionResult<()> {
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    for code in [0, 7] {
        host.observe().native_io.exit_code = Some(code);
        let child =
            super::block_on(io.spawn("literal".into(), Vec::new(), SpawnOptions::default()))?;
        assert_eq!(
            child.pid(),
            Some(host.observed().native_io.child.as_ref().unwrap().pid)
        );
        let (hold, _started, open) = gate();
        host.observe().hold_next_idle(hold);
        let mut wait = child.wait();
        assert!(
            wait.as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        open.send(()).unwrap();
        assert_eq!(
            super::block_on(wait)?,
            crate::child_process::ExitStatus {
                code: Some(code),
                signal: None
            }
        );
    }
    let child = super::block_on(io.spawn("literal".into(), Vec::new(), SpawnOptions::default()))?;
    assert!(child.kill(Some("SIGINT".into()))?);
    assert_eq!(
        super::block_on(child.wait())?,
        crate::child_process::ExitStatus {
            code: None,
            signal: Some("SIGINT".into())
        }
    );
    Ok(())
}

#[test]
fn native_child_kill_controls_the_retained_child() -> ExtensionResult<()> {
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    for selected in [None, Some("SIGKILL".to_owned())] {
        let child =
            super::block_on(io.spawn("literal".into(), Vec::new(), SpawnOptions::default()))?;
        let original = host.observed().native_io.child.as_ref().unwrap().clone();
        assert!(child.kill(selected.clone())?);
        assert!(!original.kill(None));
        let status = super::block_on(child.wait())?;
        assert_eq!(status.code, None);
        assert_eq!(
            status.signal,
            Some(selected.unwrap_or_else(|| "SIGTERM".into()))
        );
        assert!(!child.kill(None)?);
        assert_eq!(super::block_on(child.wait())?, status);
    }
    Ok(())
}

#[test]
fn native_child_group_signal_keeps_direct_fallback() -> ExtensionResult<()> {
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    let child = super::block_on(io.spawn(
        "literal".into(),
        Vec::new(),
        SpawnOptions {
            detached: true,
            ..SpawnOptions::default()
        },
    ))?;
    assert!(host.observed().native_io.launches[0].2.detached);
    let group = -i32::try_from(child.pid().unwrap()).unwrap();
    io.kill(group, Some("SIGKILL".into()))?;
    assert_eq!(
        host.observed().native_io.signals,
        [(group, Some("SIGKILL".into()))]
    );
    host.observe().native_io.error = Some("group unavailable".into());
    assert_eq!(
        io.kill(group, Some("SIGKILL".into())),
        Err("group unavailable".into())
    );
    assert!(child.kill(Some("SIGKILL".into()))?);
    assert_eq!(
        super::block_on(child.wait())?.signal.as_deref(),
        Some("SIGKILL")
    );
    Ok(())
}

#[test]
fn native_child_abort_keeps_the_original_signal() -> ExtensionResult<()> {
    use std::rc::Rc;
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    let flag = Flag::default();
    let original_signal = signal(&host, host.signal(&flag));
    let child = super::block_on(io.spawn("literal".into(), Vec::new(), SpawnOptions::default()))?;
    let retained = Rc::clone(&child);
    let subscription =
        original_signal.subscribe(Rc::new(move |_| retained.kill(None).map(|_| ())))?;
    drop(original_signal);
    assert_eq!(host.observed().bus.listeners.len(), 1);
    assert_eq!(host.abort(&flag), [Ok(())]);
    assert_eq!(
        super::block_on(child.wait())?.signal.as_deref(),
        Some("SIGTERM")
    );
    subscription.unsubscribe()?;
    assert!(host.observed().bus.listeners.is_empty());
    Ok(())
}

#[test]
fn native_child_errors_remain_catchable() -> ExtensionResult<()> {
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    for failure in ["missing executable", "invalid cwd"] {
        host.observe().native_io.error = Some(failure.into());
        assert_eq!(
            super::block_on(io.spawn("literal".into(), Vec::new(), SpawnOptions::default())).err(),
            Some(failure.into())
        );
    }
    host.observe().native_io.error = None;
    host.observe().native_io.stdout = vec![vec![0, 255]];
    host.observe().native_io.read_error = Some("read failed Ω".into());
    let child = super::block_on(io.spawn("literal".into(), Vec::new(), SpawnOptions::default()))?;
    let output = child.stdout().unwrap();
    assert_eq!(super::block_on(output.read())?, Some(vec![0, 255]));
    assert_eq!(super::block_on(output.read()), Err("read failed Ω".into()));
    let input = child.stdin().unwrap();
    super::block_on(input.end())?;
    assert_eq!(
        super::block_on(input.write(&[1])),
        Err("controlled input closed".into())
    );
    host.observe().native_io.error = Some("signal failed".into());
    assert_eq!(
        io.kill(1234, Some("bad signal".into())),
        Err("signal failed".into())
    );
    host.observe().native_io.error = None;
    host.observe().native_io.read_error = None;
    host.observe().native_io.stdout = vec![b"sibling".to_vec()];
    let sibling = super::block_on(io.spawn("literal".into(), Vec::new(), SpawnOptions::default()))?;
    assert_eq!(
        super::block_on(sibling.stdout().unwrap().read())?,
        Some(b"sibling".to_vec())
    );
    assert_eq!(super::block_on(sibling.wait())?.code, Some(7));
    Ok(())
}

#[test]
fn native_child_handles_outlive_the_callback() -> ExtensionResult<()> {
    use std::sync::Arc;
    let host = Controlled::default();
    host.observe().native_io.stdout = vec![vec![0, 255]];
    host.observe().native_io.stderr = vec![b"retained".to_vec()];
    let io = captured_api(&host)?.native_io();
    let child = super::block_on(io.spawn("literal".into(), Vec::new(), SpawnOptions::default()))?;
    let input = child.stdin().unwrap();
    let output = child.stdout().unwrap();
    let error = child.stderr().unwrap();
    let mut original = host.observe().native_io.child.take().unwrap();
    let input_released = Arc::downgrade(&original.stdin.take().unwrap().bytes);
    drop(child);
    drop(io);
    assert!(input_released.upgrade().is_some());
    super::block_on(input.write(&[0, 255]))?;
    super::block_on(input.end())?;
    assert_eq!(super::block_on(output.read())?, Some(vec![0, 255]));
    assert_eq!(super::block_on(error.read())?, Some(b"retained".to_vec()));
    drop(input);
    assert!(input_released.upgrade().is_none());
    assert!(
        original.kill(None),
        "dropping controls must not signal the child"
    );
    Ok(())
}

#[test]
fn native_fetch_forwards_the_complete_request() -> ExtensionResult<()> {
    use crate::http::Request;
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    let flag = Flag::default();
    for body in [None, Some(vec![]), Some(vec![0, 255, 226, 130, 172])] {
        let response = super::block_on(io.fetch(Request {
            url: "http://loopback/Ω".into(),
            method: body.as_ref().map(|_| "POST".into()),
            headers: vec![("X-Author".into(), " value Ω ".into())],
            body: body.clone(),
            signal: Some(signal(&host, host.signal(&flag))),
        }))?;
        assert_eq!(response.status(), 200);
        let observed = host.observed();
        let request = observed.native_http.requests.last().unwrap();
        assert_eq!(request.url, "http://loopback/Ω");
        assert_eq!(request.method, body.as_ref().map(|_| "POST".into()));
        assert_eq!(request.headers, [("X-Author".into(), " value Ω ".into())]);
        assert_eq!(request.body, body);
        assert_eq!(request.signal, Some(flag.key()));
    }
    Ok(())
}

#[test]
fn native_response_keeps_metadata_and_body_presence() -> ExtensionResult<()> {
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    for status in [200, 204] {
        host.observe().native_http.status = status;
        host.observe().native_http.status_text = "Owned reason".into();
        host.observe().native_http.headers = vec![("x-response".into(), "value".into())];
        host.observe().native_http.body = (status != 204).then(Vec::new);
        let response = super::block_on(io.fetch(crate::http::Request {
            url: "http://loopback/".into(),
            method: None,
            headers: vec![],
            body: None,
            signal: None,
        }))?;
        assert_eq!(response.status(), status);
        assert!(response.ok());
        assert_eq!(response.status_text(), "Owned reason");
        assert_eq!(response.headers(), [("x-response".into(), "value".into())]);
        if status == 204 {
            assert!(response.body().is_none());
        } else {
            let body = response.body().unwrap();
            assert!(std::rc::Rc::ptr_eq(&body, &response.body().unwrap()));
            assert_eq!(super::block_on(body.read())?, None);
        }
    }
    Ok(())
}

#[test]
fn native_response_yields_bytes_before_body_completion() -> ExtensionResult<()> {
    let host = Controlled::default();
    host.observe().native_http.body = Some(vec![vec![0, 255, 226], vec![130, 172, 1]]);
    let io = captured_api(&host)?.native_io();
    let response = super::block_on(io.fetch(crate::http::Request {
        url: "http://loopback/".into(),
        method: None,
        headers: vec![],
        body: None,
        signal: None,
    }))?;
    let body = response.body().unwrap();
    assert_eq!(super::block_on(body.read())?, Some(vec![0, 255, 226]));
    let (hold, _started, open) = gate();
    host.observe().hold_next_idle(hold);
    let mut next = body.read();
    assert!(
        next.as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    open.send(()).unwrap();
    assert_eq!(super::block_on(next)?, Some(vec![130, 172, 1]));
    assert_eq!(super::block_on(body.read())?, None);
    Ok(())
}

#[test]
fn native_response_close_keeps_caller_signal_live() -> ExtensionResult<()> {
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    let flag = Flag::default();
    let caller = signal(&host, host.signal(&flag));
    let response = super::block_on(io.fetch(crate::http::Request {
        url: "http://loopback/".into(),
        method: None,
        headers: vec![],
        body: None,
        signal: Some(super::signal_callbacks::authored(caller.clone())),
    }))?;
    let body = response.body().unwrap();
    drop(response);
    assert_eq!(host.observed().bus.listeners.len(), 1);
    body.close()?;
    assert!(!caller.aborted());
    drop(body);
    assert!(host.observed().bus.listeners.is_empty());
    assert!(!caller.aborted());
    Ok(())
}

#[test]
fn native_fetch_returns_non_success_responses() -> ExtensionResult<()> {
    let host = Controlled::default();
    host.observe().native_http.body = Some(vec![b"denied".to_vec()]);
    host.observe().native_http.text = "denied".into();
    let io = captured_api(&host)?.native_io();
    for (status, ok) in [
        (200, true),
        (204, true),
        (299, true),
        (300, false),
        (401, false),
        (404, false),
        (599, false),
    ] {
        host.observe().native_http.status = status;
        let response = super::block_on(io.fetch(crate::http::Request {
            url: "http://loopback/".into(),
            method: None,
            headers: vec![],
            body: None,
            signal: None,
        }))?;
        assert_eq!(response.status(), status);
        assert_eq!(response.ok(), ok);
        if status == 401 {
            let error = format!(
                "Token exchange failed: {}",
                super::block_on(response.text())?
            );
            assert_eq!(error, "Token exchange failed: denied");
        }
    }
    Ok(())
}

/// Supplies one request without inventing any native transport policy.
fn http_request(signal: Option<crate::AbortSignal>) -> crate::http::Request {
    crate::http::Request {
        url: "http://loopback/".into(),
        method: None,
        headers: vec![],
        body: None,
        signal,
    }
}

#[test]
fn native_response_errors_keep_their_phase() -> ExtensionResult<()> {
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    let flag = Flag::default();
    flag.abort();
    assert_eq!(
        super::block_on(io.fetch(http_request(Some(signal(&host, host.signal(&flag)))))).err(),
        Some("Aborted".into())
    );
    let flag = Flag::default();
    let (hold, _started, open) = gate();
    host.observe().hold_next_idle(hold);
    let mut fetch = io.fetch(http_request(Some(signal(&host, host.signal(&flag)))));
    assert!(
        fetch
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    flag.abort();
    open.send(()).unwrap();
    assert_eq!(super::block_on(fetch).err(), Some("Aborted".into()));
    let flag = Flag::default();
    host.observe().native_http.body = Some(vec![b"first".to_vec()]);
    let response =
        super::block_on(io.fetch(http_request(Some(signal(&host, host.signal(&flag))))))?;
    let body = response.body().unwrap();
    assert_eq!(super::block_on(body.read())?, Some(b"first".to_vec()));
    let (hold, _started, open) = gate();
    host.observe().hold_next_idle(hold);
    let mut read = body.read();
    assert!(
        read.as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    flag.abort();
    open.send(()).unwrap();
    assert_eq!(super::block_on(read), Err("Aborted".into()));
    host.observe().native_http.error = Some("connection failed Ω".into());
    assert_eq!(
        super::block_on(io.fetch(http_request(None))).err(),
        Some("connection failed Ω".into())
    );
    host.observe().native_http.error = None;
    host.observe().native_http.read_error = Some("body failed Ω".into());
    let response = super::block_on(io.fetch(http_request(None)))?;
    let body = response.body().unwrap();
    assert_eq!(super::block_on(body.read())?, Some(b"first".to_vec()));
    assert_eq!(super::block_on(body.read()), Err("body failed Ω".into()));
    assert_eq!(super::block_on(body.read())?, None);
    Ok(())
}

#[test]
fn native_response_consumers_share_one_body() -> ExtensionResult<()> {
    let host = Controlled::default();
    host.observe().native_http.text = "{\"value\":1}".into();
    host.observe().native_http.body = Some(vec![b"{\"value\":".to_vec(), b"1}".to_vec()]);
    let io = captured_api(&host)?.native_io();
    for first in ["text", "json", "read"] {
        let response = super::block_on(io.fetch(http_request(None)))?;
        let body = response.body().unwrap();
        assert!(std::rc::Rc::ptr_eq(&body, &response.body().unwrap()));
        match first {
            "text" => assert_eq!(super::block_on(response.text())?, "{\"value\":1}"),
            "json" => assert_eq!(
                super::block_on(response.json())?,
                serde_json::json!({"value":1})
            ),
            _ => assert_eq!(super::block_on(body.read())?, Some(b"{\"value\":".to_vec())),
        }
        assert!(super::block_on(response.text()).is_err());
        assert!(super::block_on(response.json()).is_err());
        if first == "read" {
            assert_eq!(super::block_on(body.read())?, Some(b"1}".to_vec()));
        }
        assert_eq!(super::block_on(body.read())?, None);
    }
    Ok(())
}

#[test]
fn native_response_json_keeps_all_value_kinds() -> ExtensionResult<()> {
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    for (text, expected) in [
        ("null", serde_json::Value::Null),
        ("\"雪\\u0000\"", serde_json::json!("雪\0")),
        ("42", serde_json::json!(42)),
        (
            "1.2345678901234567",
            serde_json::json!(1.234_567_890_123_456_7),
        ),
        ("-0.0", serde_json::json!(-0.0)),
        ("true", serde_json::json!(true)),
        ("false", serde_json::json!(false)),
        (
            "{\"2\":\"two\",\"1\":\"one\",\"a\":null}",
            serde_json::json!({"2":"two","1":"one","a":null}),
        ),
        ("[1,false,null]", serde_json::json!([1, false, null])),
        ("1e308", serde_json::json!(1e308)),
        ("5e-324", serde_json::json!(5e-324)),
        (
            "9007199254740993",
            serde_json::json!(9_007_199_254_740_993_u64),
        ),
    ] {
        host.observe().native_http.text = text.into();
        let response = super::block_on(io.fetch(http_request(None)))?;
        let value = super::block_on(response.json())?;
        assert_eq!(value, expected, "{text}");
        if text == "-0.0" {
            assert!(value.as_f64().unwrap().is_sign_negative());
        }
    }
    for text in ["", "{", "undefined"] {
        host.observe().native_http.text = text.into();
        let response = super::block_on(io.fetch(http_request(None)))?;
        assert_eq!(response.status(), 200);
        assert!(super::block_on(response.json()).is_err(), "{text}");
    }
    Ok(())
}

#[test]
fn native_response_text_uses_the_existing_decoder() -> ExtensionResult<()> {
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    for (bytes, expected) in [
        (vec![], ""),
        (b"\xef\xbb\xbfhello".to_vec(), "hello"),
        (b"\xef\xbb\xbf\xef\xbb\xbfhello".to_vec(), "\u{feff}hello"),
        (b"\0 \t\r\n".to_vec(), "\0 \t\r\n"),
        (b"\xff\xe2\x82".to_vec(), "\u{fffd}\u{fffd}"),
        ("Ω雪".as_bytes().to_vec(), "Ω雪"),
    ] {
        host.observe().native_http.body = Some(vec![bytes]);
        host.observe().native_http.text = expected.into();
        let response = super::block_on(io.fetch(http_request(None)))?;
        assert_eq!(super::block_on(response.text())?, expected);
    }
    Ok(())
}

#[test]
fn native_watch_notifies_for_the_supplied_file() -> ExtensionResult<()> {
    use std::{cell::Cell, rc::Rc};
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    let calls = Rc::new(Cell::new(0));
    let received = calls.clone();
    let watch = io.watch(
        "/outside Ω/trigger.txt".into(),
        Rc::new(move |emitter| {
            received.set(received.get() + 1);
            emitter.emit("watched", serde_json::json!("effect Ω"))
        }),
    )?;
    assert_eq!(
        host.observed().native_watch.paths,
        ["/outside Ω/trigger.txt"]
    );
    host.notify_watch(0)?;
    host.notify_watch(0)?;
    assert_eq!(calls.get(), 2);
    assert!(
        host.observed()
            .bus
            .emissions
            .contains(&("watched".into(), serde_json::json!("effect Ω")))
    );
    watch.close()?;
    Ok(())
}

#[test]
fn native_watch_close_releases_its_registration() -> ExtensionResult<()> {
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    for close in [false, true] {
        let captured = Rc::new(());
        let released = Rc::downgrade(&captured);
        let calls = Rc::new(Cell::new(0));
        let received = calls.clone();
        let watch = io.watch(
            "/outside/trigger".into(),
            Rc::new(move |_| {
                let _ = &captured;
                received.set(received.get() + 1);
                Ok(())
            }),
        )?;
        let index = host.observed().native_watch.watches.len() - 1;
        let alias = watch.clone();
        drop(watch);
        host.notify_watch(index)?;
        assert_eq!(calls.get(), 1);
        assert!(released.upgrade().is_some());
        if close {
            alias.close()?;
            alias.close()?;
            assert!(released.upgrade().is_none());
        }
        drop(alias);
        assert!(released.upgrade().is_none());
        host.notify_watch(index)?;
        assert_eq!(calls.get(), 1);
    }
    let slot = Rc::new(RefCell::new(None::<Rc<dyn crate::fs::FsWatcher>>));
    let retained = slot.clone();
    let captured = Rc::new(());
    let released = Rc::downgrade(&captured);
    *slot.borrow_mut() = Some(io.watch(
        "/self-close".into(),
        Rc::new(move |_| {
            let _ = &captured;
            retained.borrow().as_ref().unwrap().close()
        }),
    )?);
    host.notify_watch(2)?;
    assert!(released.upgrade().is_none());
    slot.borrow().as_ref().unwrap().close()?;
    Ok(())
}

#[test]
fn native_watch_failures_do_not_leak_callbacks() -> ExtensionResult<()> {
    use std::{cell::Cell, rc::Rc};
    let host = Controlled::default();
    let io = captured_api(&host)?.native_io();
    let calls = Rc::new(Cell::new(0));
    let received = calls.clone();
    let sibling = io.watch(
        "/outside/live".into(),
        Rc::new(move |_| {
            received.set(received.get() + 1);
            Ok(())
        }),
    )?;
    let capture = Rc::new(());
    let released = Rc::downgrade(&capture);
    host.observe().native_watch.error = Some("watch creation failed Ω".into());
    let failure = io.watch(
        "/outside/missing".into(),
        Rc::new(move |_| {
            let _ = &capture;
            Ok(())
        }),
    );
    assert_eq!(failure.err(), Some("watch creation failed Ω".into()));
    assert!(released.upgrade().is_none());
    assert_eq!(host.observed().native_watch.watches.len(), 1);
    host.notify_watch(0)?;
    assert_eq!(calls.get(), 1);
    host.observe().native_watch.error = None;
    let next = io.watch("/outside/recovered".into(), Rc::new(|_| Ok(())))?;
    host.notify_watch(1)?;
    next.close()?;
    sibling.close()?;
    Ok(())
}
