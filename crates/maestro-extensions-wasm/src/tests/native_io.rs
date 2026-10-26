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
