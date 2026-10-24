//! Native termination signals, their delivery to a mode that shuts down on them, and the
//! default action a second one takes once graceful shutdown has started.
use std::io;
use std::rc::Rc;
#[cfg(unix)]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// A termination signal a long-running process shuts down on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminationSignal {
    /// A request to terminate.
    Terminate,
    /// The controlling connection hung up.
    Hangup,
}

impl TerminationSignal {
    /// The POSIX signal number.
    #[must_use]
    pub fn as_raw(self) -> i32 {
        match self {
            Self::Terminate => 15,
            Self::Hangup => 1,
        }
    }
}

/// The signals this host delivers: termination everywhere, and a hangup off Windows.
#[must_use]
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

/// Delivers each termination signal to `callback`; the returned operation stops the delivery
/// and is idempotent.
///
/// The delivery runs as tasks of the calling thread, so the caller drives it from a Tokio
/// runtime with the signal driver enabled.
///
/// # Errors
/// Returns the native registration failure.
#[cfg(unix)]
pub fn register_signals(callback: Rc<dyn Fn(TerminationSignal)>) -> io::Result<Box<dyn Fn()>> {
    let mut signals = signals_for_host().into_iter();
    let last = signals.next_back();
    let mut deliveries = Vec::new();
    for signal in signals {
        deliveries.push(deliver_on(signal, Rc::clone(&callback))?);
    }
    if let Some(signal) = last {
        deliveries.push(deliver_on(signal, callback)?);
    }
    Ok(Box::new(move || {
        deliveries.iter().for_each(tokio::task::JoinHandle::abort);
    }))
}

/// Starts delivering one native signal to `callback`, which its task then owns.
#[cfg(unix)]
fn deliver_on(
    signal: TerminationSignal,
    callback: Rc<dyn Fn(TerminationSignal)>,
) -> io::Result<tokio::task::JoinHandle<()>> {
    let stream = tokio::signal::unix::signal(kind(signal))?;
    Ok(tokio::task::spawn_local(deliver(stream, signal, callback)))
}

/// A host without POSIX signal delivery takes no registration and leaves its own console
/// events alone.
///
/// # Errors
/// Never: this host has no native termination delivery to register.
#[cfg(not(unix))]
pub fn register_signals(_callback: Rc<dyn Fn(TerminationSignal)>) -> io::Result<Box<dyn Fn()>> {
    Ok(Box::new(|| ()))
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

/// The native stream of `signal`.
#[cfg(unix)]
fn kind(signal: TerminationSignal) -> tokio::signal::unix::SignalKind {
    match signal {
        TerminationSignal::Terminate => tokio::signal::unix::SignalKind::terminate(),
        TerminationSignal::Hangup => tokio::signal::unix::SignalKind::hangup(),
    }
}

/// The native default action of each prepared signal, taken once [`restore`] is called.
///
/// A process that handles termination signals itself keeps their delivery registered for
/// its whole lifetime; restoring makes a later signal terminate the process as if no handler
/// had been installed, even while the process is blocked in shutdown.
///
/// [`restore`]: DefaultSignalActions::restore
#[cfg(unix)]
pub struct DefaultSignalActions {
    /// Set once default termination is restored.
    restored: Arc<AtomicBool>,
}

#[cfg(unix)]
impl DefaultSignalActions {
    /// Prepares `signals` without changing their current delivery.
    ///
    /// # Errors
    /// Returns the native handler registration failure.
    pub fn new(signals: &[TerminationSignal]) -> std::io::Result<Self> {
        let restored = Arc::new(AtomicBool::new(false));
        for signal in signals {
            signal_hook::flag::register_conditional_default(
                signal.as_raw(),
                Arc::clone(&restored),
            )?;
        }
        Ok(Self { restored })
    }

    /// Makes every later prepared signal take its native default action.
    pub fn restore(&self) {
        self.restored.store(true, Ordering::SeqCst);
    }
}
