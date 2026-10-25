//! Termination delivery to the modes that shut down on it, driven through real signals.
#![cfg(test)]
#![cfg(unix)]
use maestro_process::TerminationSignal;
use maestro_process::termination::{register_signals, signals_for_host};
use std::io;
use std::rc::Rc;
use tokio::sync::mpsc;

/// Raises `signal` for this process, which delivers it to the registered subscribers.
#[cfg(unix)]
fn raise(signal: TerminationSignal) {
    use rustix::process::{Pid, Signal, kill_process};
    let native = match signal {
        TerminationSignal::Terminate => Signal::TERM,
        TerminationSignal::Hangup => Signal::HUP,
        TerminationSignal::Interrupt => Signal::INT,
    };
    let own = Pid::from_raw(i32::try_from(std::process::id()).expect("a native process id"))
        .expect("a valid process id");
    assert_eq!(
        kill_process(own, native),
        Ok(()),
        "the signal reached this process"
    );
}

/// One caller's registration: the signals it received and the operation stopping delivery.
struct Caller {
    /// Every signal the registration delivered, in order.
    received: mpsc::UnboundedReceiver<TerminationSignal>,
    /// Stops delivery; calling it again changes nothing.
    remove: Box<dyn Fn()>,
}

/// Registers one caller's termination delivery.
async fn caller() -> io::Result<Caller> {
    let (sender, received) = mpsc::unbounded_channel();
    let remove = register_signals(Rc::new(move |signal| {
        let _ = sender.send(signal);
    }))?;
    tokio::task::yield_now().await;
    Ok(Caller { received, remove })
}

#[test]
fn termination_subscription_delivers_and_removes_for_both_callers() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a current-thread runtime");
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let expected = if cfg!(unix) {
            vec![TerminationSignal::Terminate, TerminationSignal::Hangup]
        } else {
            vec![TerminationSignal::Terminate]
        };
        assert_eq!(
            signals_for_host(),
            expected,
            "termination everywhere and a hangup off Windows"
        );
        let mut print = caller().await.expect("the print registration");
        let mut interactive = caller().await.expect("the interactive registration");
        for signal in expected {
            raise(signal);
            assert_eq!(
                print.received.recv().await,
                Some(signal),
                "the print caller received the signal"
            );
            assert_eq!(
                interactive.received.recv().await,
                Some(signal),
                "the interactive caller received the same signal"
            );
        }
        (print.remove)();
        (print.remove)();
        raise(TerminationSignal::Terminate);
        assert_eq!(
            interactive.received.recv().await,
            Some(TerminationSignal::Terminate),
            "the remaining registration still delivers"
        );
        assert!(
            print.received.try_recv().is_err(),
            "a removed registration delivers nothing further"
        );
        (interactive.remove)();
    });
}

/// Removing the final delivery during the first signal must not apply that signal's
/// default action while its native handler is still running on another thread.
#[test]
fn first_signal_removal_does_not_terminate_in_flight_delivery() {
    use maestro_test_process::{TestProcess, selection};
    if !selection().is_child() {
        let output =
            TestProcess::current("first_signal_removal_does_not_terminate_in_flight_delivery")
                .unwrap()
                .case("child")
                .nocapture()
                .output()
                .unwrap();
        assert!(output.status.success(), "{}", output.status);
        return;
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        for _ in 0..1000 {
            let mut registration = caller().await.unwrap();
            let sender = std::thread::spawn(|| {
                signal_hook::low_level::raise(15).unwrap();
            });
            assert_eq!(
                registration.received.recv().await,
                Some(TerminationSignal::Terminate)
            );
            (registration.remove)();
            sender.join().unwrap();
        }
    });
}

#[test]
fn last_removal_restores_native_termination() {
    use maestro_test_process::{TestProcess, selection};
    use std::os::unix::process::ExitStatusExt;
    if selection().is_child() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        tokio::task::LocalSet::new().block_on(&runtime, async {
            let registration = caller().await.unwrap();
            (registration.remove)();
            (registration.remove)();
            let signal = if selection().runs_case("term") {
                TerminationSignal::Terminate
            } else {
                TerminationSignal::Hangup
            };
            raise(signal);
        });
        return;
    }
    for (case, expected) in [("term", 15), ("hup", 1)] {
        let output = TestProcess::current("last_removal_restores_native_termination")
            .unwrap()
            .case(case)
            .nocapture()
            .output()
            .unwrap();
        assert_eq!(
            output.status.signal(),
            Some(expected),
            "final removal restores the native action for {case}"
        );
    }
}

#[test]
fn reopen_and_independent_registrations_remain_live() {
    if !maestro_test_process::selection().is_child() {
        let output = maestro_test_process::TestProcess::current(
            "reopen_and_independent_registrations_remain_live",
        )
        .unwrap()
        .case("child")
        .nocapture()
        .output()
        .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let old = caller().await.unwrap();
        (old.remove)();
        let mut first = caller().await.unwrap();
        let mut second = caller().await.unwrap();
        (old.remove)();
        for signal in signals_for_host() {
            raise(signal);
            assert_eq!(first.received.recv().await, Some(signal));
            assert_eq!(second.received.recv().await, Some(signal));
        }
        raise(TerminationSignal::Terminate);
        (first.remove)();
        (first.remove)();
        assert_eq!(
            second.received.recv().await,
            Some(TerminationSignal::Terminate)
        );
        assert!(
            first.received.try_recv().is_err(),
            "removal cancels already-ready delivery"
        );
        for signal in signals_for_host() {
            raise(signal);
            assert_eq!(second.received.recv().await, Some(signal));
            assert!(first.received.try_recv().is_err());
        }
        (second.remove)();
    });
}
