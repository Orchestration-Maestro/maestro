//! Native termination-signal delivery for a print run.
use maestro_app::TerminationSignal;
use std::{io, rc::Rc};

/// The signal delivery and process exit of a print run.
pub trait SignalPort {
    /// Delivers each termination signal to `callback`; the returned operation stops the
    /// delivery.
    ///
    /// # Errors
    /// Returns the native registration failure.
    fn register(&self, callback: Rc<dyn Fn(TerminationSignal)>) -> io::Result<Box<dyn Fn()>>;

    /// Ends the process with `code`.
    fn exit(&self, code: i32);
}

/// The host's own signal delivery and process exit.
pub struct NativeSignals;

impl SignalPort for NativeSignals {
    #[cfg(unix)]
    fn register(&self, callback: Rc<dyn Fn(TerminationSignal)>) -> io::Result<Box<dyn Fn()>> {
        let mut handles = Vec::new();
        for signal in signals_for_host() {
            let stream = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::from_raw(
                signal.as_raw(),
            ))?;
            handles.push(tokio::task::spawn_local(deliver(
                stream,
                signal,
                Rc::clone(&callback),
            )));
        }
        Ok(Box::new(move || {
            handles.iter().for_each(tokio::task::JoinHandle::abort);
        }))
    }

    /// A host without POSIX signal delivery takes no registration and leaves its own
    /// console events alone.
    #[cfg(not(unix))]
    fn register(&self, _callback: Rc<dyn Fn(TerminationSignal)>) -> io::Result<Box<dyn Fn()>> {
        Ok(Box::new(|| ()))
    }

    fn exit(&self, code: i32) {
        std::process::exit(code);
    }
}

/// The signals this host delivers: termination everywhere, and a hangup off Windows.
///
/// A host that can raise no POSIX signal binds no stream, so this table compiles where
/// delivery is possible and in the tests that assert it.
#[cfg(any(unix, test))]
pub fn signals_for_host() -> Vec<TerminationSignal> {
    #[cfg(unix)]
    {
        vec![TerminationSignal::Terminate, TerminationSignal::Hangup]
    }
    #[cfg(not(unix))]
    {
        vec![TerminationSignal::Terminate]
    }
}

/// Delivers every signal the stream reports to `callback`.
#[cfg(unix)]
async fn deliver(
    mut stream: tokio::signal::unix::Signal,
    signal: TerminationSignal,
    callback: Rc<dyn Fn(TerminationSignal)>,
) {
    while stream.recv().await.is_some() {
        callback(signal);
    }
}
