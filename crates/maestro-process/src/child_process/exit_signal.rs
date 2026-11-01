//! Native names of the signals that end a child process.

/// Name the signal that ended `status`, when this host reports one.
///
/// A numeric exit and a signal this host does not name both return `None`.
#[cfg(unix)]
#[must_use]
pub fn exit_signal_name(status: std::process::ExitStatus) -> Option<&'static str> {
    use std::os::unix::process::ExitStatusExt;
    let signal = status.signal();
    NAMES
        .iter()
        .find(|(candidate, _)| Some(candidate.as_raw()) == signal)
        .map(|(_, name)| *name)
}

/// Name the signal that ended `_status`, which this host never reports.
#[cfg(not(unix))]
#[must_use]
pub fn exit_signal_name(_status: std::process::ExitStatus) -> Option<&'static str> {
    None
}

#[cfg(unix)]
use rustix::process::Signal;

/// Native signal names shared by exit observations and signal delivery.
#[cfg(unix)]
pub(super) const NAMES: &[(rustix::process::Signal, &str)] = &[
    (Signal::HUP, "SIGHUP"),
    (Signal::INT, "SIGINT"),
    (Signal::QUIT, "SIGQUIT"),
    (Signal::ILL, "SIGILL"),
    (Signal::TRAP, "SIGTRAP"),
    (Signal::ABORT, "SIGABRT"),
    (Signal::BUS, "SIGBUS"),
    (Signal::FPE, "SIGFPE"),
    (Signal::KILL, "SIGKILL"),
    (Signal::USR1, "SIGUSR1"),
    (Signal::SEGV, "SIGSEGV"),
    (Signal::USR2, "SIGUSR2"),
    (Signal::PIPE, "SIGPIPE"),
    (Signal::ALARM, "SIGALRM"),
    (Signal::TERM, "SIGTERM"),
    (Signal::CHILD, "SIGCHLD"),
    (Signal::CONT, "SIGCONT"),
    (Signal::STOP, "SIGSTOP"),
    (Signal::TSTP, "SIGTSTP"),
    (Signal::TTIN, "SIGTTIN"),
    (Signal::TTOU, "SIGTTOU"),
    (Signal::URG, "SIGURG"),
    (Signal::XCPU, "SIGXCPU"),
    (Signal::XFSZ, "SIGXFSZ"),
    (Signal::VTALARM, "SIGVTALRM"),
    (Signal::PROF, "SIGPROF"),
    (Signal::WINCH, "SIGWINCH"),
    (Signal::SYS, "SIGSYS"),
    (Signal::IO, "SIGIO"),
    #[cfg(target_os = "linux")]
    (Signal::STKFLT, "SIGSTKFLT"),
    #[cfg(target_os = "linux")]
    (Signal::POWER, "SIGPWR"),
    #[cfg(target_os = "macos")]
    (Signal::EMT, "SIGEMT"),
    #[cfg(target_os = "macos")]
    (Signal::INFO, "SIGINFO"),
];
