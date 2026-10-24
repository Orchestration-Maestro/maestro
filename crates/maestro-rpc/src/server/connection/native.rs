//! Termination signal delivery of the running process.
use maestro_app::TerminationSignal;
use maestro_app::connection_runner::RpcProcessPort;
use std::{io, rc::Rc};

/// The running process's termination signals: SIGTERM and SIGHUP on Unix, none elsewhere.
pub struct NativeRpcProcess;

#[cfg(unix)]
impl RpcProcessPort for NativeRpcProcess {
    /// Delivers each signal until stopped; once stopped, a later signal takes its native
    /// default action, even while shutdown blocks.
    fn register_signals(
        &self,
        callback: Rc<dyn Fn(TerminationSignal)>,
    ) -> io::Result<Box<dyn Fn()>> {
        use tokio::signal::unix::{SignalKind, signal};
        let signals = [TerminationSignal::Terminate, TerminationSignal::Hangup];
        let defaults = maestro_app::DefaultSignalActions::new(&signals)?;
        let mut deliveries = Vec::new();
        for kind in signals {
            let received = signal(SignalKind::from_raw(kind.as_raw()))?;
            let delivery = deliver(received, kind, Rc::clone(&callback));
            deliveries.push(tokio::task::spawn_local(delivery));
        }
        Ok(Box::new(move || {
            defaults.restore();
            deliveries.iter().for_each(tokio::task::JoinHandle::abort);
        }))
    }
}

/// Calls `callback` with `kind` each time the signal arrives.
#[cfg(unix)]
async fn deliver(
    mut received: tokio::signal::unix::Signal,
    kind: TerminationSignal,
    callback: Rc<dyn Fn(TerminationSignal)>,
) {
    while received.recv().await.is_some() {
        callback(kind);
    }
}

#[cfg(not(unix))]
impl RpcProcessPort for NativeRpcProcess {
    fn register_signals(
        &self,
        _callback: Rc<dyn Fn(TerminationSignal)>,
    ) -> io::Result<Box<dyn Fn()>> {
        Ok(Box::new(|| {}))
    }
}
