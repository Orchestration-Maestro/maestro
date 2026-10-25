//! Native termination signals and the default action a prepared one takes once its
//! handling ends. Delivery to a shutting-down mode is shared with native callers.
#[cfg(unix)]
use std::cell::Cell;
use std::io;
use std::rc::Rc;
#[cfg(unix)]
use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
};

/// A termination signal a long-running process shuts down on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminationSignal {
    /// A request to terminate.
    Terminate,
    /// The controlling connection hung up.
    Hangup,
    /// An interrupt from the keyboard.
    Interrupt,
}

impl TerminationSignal {
    /// The POSIX signal number.
    #[must_use]
    pub fn as_raw(self) -> i32 {
        match self {
            Self::Terminate => 15,
            Self::Hangup => 1,
            Self::Interrupt => 2,
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
/// runtime with the signal driver enabled. Removing the last live registration restores
/// the native TERM/HUP default actions; reopening defers those same prepared actions again.
/// The default-action check precedes delivery, so removing a registration from its
/// callback cannot terminate the process with the signal already being handled.
/// Dropping the removal handle alone does not unregister the callback.
///
/// # Errors
/// Returns the native registration failure.
#[cfg(unix)]
pub fn register_signals(callback: Rc<dyn Fn(TerminationSignal)>) -> io::Result<Box<dyn Fn()>> {
    acquire_defaults()?;
    let signals = signals_for_host()
        .into_iter()
        .map(|signal| {
            let kind = tokio::signal::unix::SignalKind::from_raw(signal.as_raw());
            tokio::signal::unix::signal(kind).map(|stream| (signal, stream))
        })
        .collect::<io::Result<Vec<_>>>()
        .inspect_err(|_| release_defaults())?;
    let deliveries = signals
        .into_iter()
        .map(move |(signal, stream)| {
            tokio::task::spawn_local(deliver(stream, signal, Rc::clone(&callback)))
        })
        .collect::<Vec<_>>();
    let removed = Cell::new(false);
    Ok(Box::new(move || {
        if !removed.replace(true) {
            deliveries.iter().for_each(tokio::task::JoinHandle::abort);
            release_defaults();
        }
    }))
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

/// The native default action of each prepared signal, taken once [`restore`] is called.
///
/// A process that handles termination signals itself keeps their delivery registered for
/// its whole lifetime; restoring makes a later signal terminate the process as if no handler
/// had been installed, even while the process is blocked in shutdown.
///
/// [`restore`]: DefaultSignalActions::restore
#[cfg(unix)]
#[derive(Clone)]
pub struct DefaultSignalActions {
    /// Set once default termination is restored.
    restored: Arc<AtomicBool>,
}

#[cfg(unix)]
impl DefaultSignalActions {
    /// Prepares `signals`; until [`restore`] is called, the prepared handler does nothing,
    /// so a signal no other handler receives is ignored.
    ///
    /// [`restore`]: DefaultSignalActions::restore
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

    /// Makes every later prepared signal do nothing again, until the next [`restore`].
    ///
    /// [`restore`]: DefaultSignalActions::restore
    pub fn defer(&self) {
        self.restored.store(false, Ordering::SeqCst);
    }
}

/// The prepared host defaults and the number of deliveries that still own them.
#[cfg(unix)]
struct TerminationDefaults {
    /// The one set of native conditional actions for TERM and HUP.
    actions: DefaultSignalActions,
    /// The registrations whose removal has not run.
    registrations: usize,
}

/// The prepared TERM/HUP actions, retained for every later registration.
#[cfg(unix)]
static TERMINATION_DEFAULTS: Mutex<Option<TerminationDefaults>> = Mutex::new(None);

/// Defers native termination while another subscription is admitted.
#[cfg(unix)]
fn acquire_defaults() -> io::Result<()> {
    let mut slot = TERMINATION_DEFAULTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if slot.is_none() {
        *slot = Some(TerminationDefaults {
            actions: DefaultSignalActions::new(&signals_for_host())?,
            registrations: 0,
        });
    }
    if let Some(state) = slot.as_mut() {
        state.actions.defer();
        state.registrations += 1;
    }
    Ok(())
}

/// Restores native termination after the final subscription stops.
#[cfg(unix)]
fn release_defaults() {
    let mut slot = TERMINATION_DEFAULTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if let Some(state) = slot.as_mut() {
        state.registrations -= 1;
        if state.registrations == 0 {
            state.actions.restore();
        }
    }
}
