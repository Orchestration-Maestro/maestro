//! Termination signal delivery of the running process.
use maestro_app::TerminationSignal;
#[cfg(unix)]
use maestro_app::register_signals;
use maestro_app::rpc_mode::RpcProcessPort;
use std::{io, rc::Rc};

/// The running process's termination signals: SIGTERM and SIGHUP on Unix, none elsewhere.
pub struct NativeRpcProcess;

#[cfg(unix)]
impl RpcProcessPort for NativeRpcProcess {
    /// Delivers each signal until stopped; once the last live registration is stopped, a
    /// later signal takes its native default action, even while shutdown blocks.
    fn register_signals(
        &self,
        callback: Rc<dyn Fn(TerminationSignal)>,
    ) -> io::Result<Box<dyn Fn()>> {
        register_signals(callback)
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
