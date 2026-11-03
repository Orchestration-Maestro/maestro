//! The typed native setup one isolated child applies before its body.
//!
//! A caller names the effects when it builds the launch. The parent writes the request into
//! the child's environment; the child reads it and applies it as its first operation, then
//! holds the returned guard. Nothing here changes the parent process's modes, limits,
//! session or environment.

use std::fmt;
use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rustix::fs::{Mode, RawMode};
use rustix::process::{Resource, Rlimit, getrlimit, setrlimit};

/// The variable that carries one launch's setup request into the child.
pub(crate) const SETUP: &str = "MAESTRO_TEST_SETUP";

/// The signal the kernel raises when a write passes the file-size limit.
const FILE_SIZE_SIGNAL: i32 = signal_hook::consts::SIGXFSZ;

/// Every permission bit of a file mode.
const PERMISSIONS: u32 = 0o777;

/// The effects one isolated child applies before its body.
///
/// The effects live with the child, so the parent keeps its own file creation mode,
/// process-count limit, session and environment. Unix only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IsolatedSetup {
    /// The permission bits this child's file creation masks.
    umask: Option<u32>,
    /// The process-count limit this child runs under.
    processes: Option<u64>,
    /// Whether a write past the file-size limit continues instead of ending the child.
    file_size_continues: bool,
    /// Whether this child leads its own session.
    new_session: bool,
}

impl IsolatedSetup {
    /// A request with no effects.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            umask: None,
            processes: None,
            file_size_continues: false,
            new_session: false,
        }
    }

    /// Mask this child's file creation with `mask`, given in permission bits.
    #[must_use]
    pub const fn umask(self, mask: u32) -> Self {
        Self {
            umask: Some(mask),
            ..self
        }
    }

    /// Run this child under a process-count limit of `processes`.
    #[must_use]
    pub const fn process_limit(self, processes: u64) -> Self {
        Self {
            processes: Some(processes),
            ..self
        }
    }

    /// Let a write past the file-size limit fail with its own error instead of ending the
    /// child.
    ///
    /// The child keeps a signal handler, not an ignored disposition, so nothing promises that
    /// a program it starts later inherits the handling.
    #[must_use]
    pub const fn file_size_continues(self) -> Self {
        Self {
            file_size_continues: true,
            ..self
        }
    }

    /// Make this child the leader of its own session.
    #[must_use]
    pub const fn new_session(self) -> Self {
        Self {
            new_session: true,
            ..self
        }
    }

    /// Apply the request this child was started with.
    ///
    /// A launch that requested no effect, and one that carries no request at all, applies
    /// nothing.
    ///
    /// # Errors
    ///
    /// Returns a request this owner does not implement, or an effect the operating system
    /// refused.
    ///
    /// # Examples
    ///
    /// Run this at the start of the isolated child's body and retain the guard until exit.
    ///
    /// ```no_run
    /// use maestro_test_process::IsolatedSetup;
    ///
    /// let _setup = IsolatedSetup::apply_requested()?;
    /// // The child's operations run here while `_setup` remains in scope.
    /// # Ok::<(), maestro_test_process::SetupError>(())
    /// ```
    pub fn apply_requested() -> Result<SetupGuard, SetupError> {
        let Some(text) = std::env::var_os(SETUP) else {
            return Self::new().apply();
        };
        let text = text
            .into_string()
            .map_err(|_| SetupError::Entry(SETUP.to_owned()))?;
        Self::parse(&text)?.apply()
    }

    /// Apply this request in the calling child and return its retained guard.
    ///
    /// The effects apply in one order: the umask, then the session, then the file-size
    /// handling, then the process-count limit. Returns the effect the operating system refused.
    fn apply(self) -> Result<SetupGuard, SetupError> {
        if let Some(mask) = self.umask {
            let mode = RawMode::try_from(mask)
                .map_err(|_| SetupError::Entry(format!("umask={mask:03o}")))?;
            rustix::process::umask(Mode::from_bits_retain(mode));
        }
        if self.new_session {
            new_session()?;
        }
        let registration = self
            .file_size_continues
            .then(register_file_size)
            .transpose()?;
        if let Some(processes) = self.processes {
            limit_processes(processes)?;
        }
        Ok(SetupGuard { registration })
    }

    /// The request `text` describes.
    ///
    /// Entries are separated by commas; each is an effect name, or an effect name and its
    /// value. A later entry replaces the same effect named earlier.
    ///
    /// # Errors
    ///
    /// Returns an entry this owner does not implement, including an effect whose value is
    /// missing or not a number of its kind.
    fn parse(text: &str) -> Result<Self, SetupError> {
        let mut setup = Self::new();
        if text.is_empty() {
            return Ok(setup);
        }
        for entry in text.split(',') {
            setup = setup.entry(entry)?;
        }
        Ok(setup)
    }

    /// This request with the one `entry` of a request text applied.
    ///
    /// # Errors
    ///
    /// Returns an entry this owner does not implement.
    fn entry(self, entry: &str) -> Result<Self, SetupError> {
        let (name, value) = entry
            .split_once('=')
            .map_or((entry, None), |(name, value)| (name, Some(value)));
        match (name, value) {
            ("umask", Some(value)) => Ok(Self {
                umask: Some(mask(value)?),
                ..self
            }),
            ("processes", Some(value)) => Ok(Self {
                processes: Some(count(value)?),
                ..self
            }),
            ("file-size-continues", None) => Ok(Self {
                file_size_continues: true,
                ..self
            }),
            ("new-session", None) => Ok(Self {
                new_session: true,
                ..self
            }),
            _ => Err(SetupError::Entry(entry.to_owned())),
        }
    }

    /// The request text this parent writes into the child's environment.
    pub(crate) fn encode(self) -> String {
        let mut entries: Vec<String> = Vec::new();
        if let Some(mask) = self.umask {
            entries.push(format!("umask={mask:03o}"));
        }
        if let Some(processes) = self.processes {
            entries.push(format!("processes={processes}"));
        }
        if self.file_size_continues {
            entries.push("file-size-continues".to_owned());
        }
        if self.new_session {
            entries.push("new-session".to_owned());
        }
        entries.join(",")
    }
}

/// The permission bits `text` names in octal.
///
/// # Errors
///
/// Returns a text that is not octal permission bits.
fn mask(text: &str) -> Result<u32, SetupError> {
    let bits = u32::from_str_radix(text, 8).map_err(|_| SetupError::Entry(text.to_owned()))?;
    if bits & !PERMISSIONS == 0 {
        Ok(bits)
    } else {
        Err(SetupError::Entry(text.to_owned()))
    }
}

/// The count `text` names.
///
/// # Errors
///
/// Returns a text that is not a decimal count.
fn count(text: &str) -> Result<u64, SetupError> {
    text.parse().map_err(|_| SetupError::Entry(text.to_owned()))
}

/// Make the calling child the leader of its own session.
///
/// # Errors
///
/// Returns the operating system's refusal, such as a child that already leads a group.
fn new_session() -> Result<(), SetupError> {
    rustix::process::setsid()
        .map(|_| ())
        .map_err(|source| SetupError::Refused {
            effect: "new-session",
            source: source.into(),
        })
}

/// Make a write past the file-size limit fail instead of ending the calling child.
///
/// # Errors
///
/// Returns the operating system's refusal to register the handler.
fn register_file_size() -> Result<signal_hook::SigId, SetupError> {
    signal_hook::flag::register(FILE_SIZE_SIGNAL, Arc::new(AtomicBool::new(false))).map_err(
        |source| SetupError::Refused {
            effect: "file-size",
            source,
        },
    )
}

/// Lower the calling child's process-count limit to `processes`.
///
/// A limit above the hard maximum is lowered to that maximum, which no unprivileged process
/// may pass.
///
/// # Errors
///
/// Returns the operating system's refusal to set the limit.
fn limit_processes(processes: u64) -> Result<(), SetupError> {
    let maximum = getrlimit(Resource::Nproc).maximum;
    let current = Some(maximum.map_or(processes, |maximum| processes.min(maximum)));
    setrlimit(Resource::Nproc, Rlimit { current, maximum }).map_err(|source| SetupError::Refused {
        effect: "processes",
        source: source.into(),
    })
}

/// The setup one isolated child holds for the rest of its run.
///
/// The umask, session and limit stay with the process; dropping the guard withdraws the
/// file-size registration.
#[derive(Debug)]
pub struct SetupGuard {
    /// The file-size registration to withdraw.
    registration: Option<signal_hook::SigId>,
}

impl Drop for SetupGuard {
    /// Withdraws this child's file-size registration.
    fn drop(&mut self) {
        if let Some(registration) = self.registration {
            signal_hook::low_level::unregister(registration);
        }
    }
}

/// A setup request that could not be applied.
#[derive(Debug)]
pub enum SetupError {
    /// The request holds an entry this owner does not implement.
    Entry(String),
    /// The operating system refused one effect.
    Refused {
        /// The effect that was refused.
        effect: &'static str,
        /// The operating system's error.
        source: io::Error,
    },
}

impl fmt::Display for SetupError {
    /// Names the refused entry or effect.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Entry(entry) => write!(
                f,
                "the isolated setup request holds `{entry}`, which names no effect this owner implements"
            ),
            Self::Refused { effect, .. } => {
                write!(f, "the isolated setup could not apply {effect}")
            }
        }
    }
}

impl std::error::Error for SetupError {
    /// The operating system's error behind a refused effect.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Entry(_) => None,
            Self::Refused { source, .. } => Some(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{IsolatedSetup, SetupError};

    /// Every file-size/session combination retains the supplied mask and process limit.
    fn assert_effects_roundtrip(mask: Option<u32>, processes: Option<u64>) {
        let mut setup = IsolatedSetup::new();
        if let Some(mask) = mask {
            setup = setup.umask(mask);
        }
        if let Some(processes) = processes {
            setup = setup.process_limit(processes);
        }
        for (file_size_continues, new_session) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            let mut request = setup;
            if file_size_continues {
                request = request.file_size_continues();
            }
            if new_session {
                request = request.new_session();
            }
            let text = request.encode();
            assert_eq!(IsolatedSetup::parse(&text).unwrap(), request, "`{text}`");
        }
    }

    #[test]
    fn a_request_text_names_every_effect_it_carries() {
        let masks = [None, Some(0o000), Some(0o022), Some(0o777)];
        let processes = [None, Some(1), Some(4096)];
        for mask in masks {
            for processes in processes {
                assert_effects_roundtrip(mask, processes);
            }
        }
    }

    #[test]
    fn a_request_names_no_effect_it_cannot_apply() {
        assert_eq!(IsolatedSetup::parse("").unwrap(), IsolatedSetup::new());
        for entry in [
            "umask=888",
            "umask=1777",
            "umask",
            "processes=x",
            "unknown=1",
            "",
        ] {
            let parsed = IsolatedSetup::parse(&format!("new-session,{entry}"));
            assert!(
                matches!(parsed, Err(SetupError::Entry(_))),
                "{entry}: {parsed:?}"
            );
        }
    }
}
