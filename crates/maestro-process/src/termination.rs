//! Native termination signals and the default action a second one takes once graceful
//! shutdown has started.
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
