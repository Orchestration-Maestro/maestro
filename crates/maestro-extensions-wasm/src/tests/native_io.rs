//! Native wait forwarding through the controlled author imports.
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
use crate::{ExtensionResult, component_adapter::signal};
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
        let signal = signal(&host, host.signal(&flag));
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
    use crate::child_process::SpawnOptions;
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
    Ok(())
}
