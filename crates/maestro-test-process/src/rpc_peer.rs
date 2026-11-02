//! Finite application RPC peer scenarios served by the prepared native fixture.

use std::io;
use std::path::Path;
use std::str::FromStr;

/// Install a peer selected by `scenario` from the prepared `MAESTRO_NATIVE_FIXTURE` artifact.
///
/// The caller owns `entry` and its sibling `.fixture` record. The parent directory must exist.
///
/// # Errors
///
/// Returns an error when preparation is missing, executable installation fails or the record
/// cannot be written.
pub fn install(entry: &Path, scenario: Scenario) -> io::Result<()> {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .ok_or_else(|| io::Error::other("run just prepare-tests, then cargo nextest"))?;
    crate::install_executable(Path::new(&artifact), entry)?;
    let mut record = entry.as_os_str().to_owned();
    record.push(".fixture");
    std::fs::write(record, format!("rpc-peer\n{}\n", scenario.name()))
}

/// A native peer's protocol or lifecycle behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    /// Serve an interactive session with streamed turn records.
    Session,
    /// Observe termination separately from the interactive executor's input closure.
    IdleInterrupt,
    /// Record one launch, announce readiness and drain input.
    Launch,
    /// Capture raw commands and answer from supplied data.
    Serve,
    /// Report native operands, directory and environment.
    Report,
    /// Report the actual native executable path.
    Entry,
    /// Write a startup diagnostic and exit with code seven.
    BootFailure,
    /// End by the native kill signal.
    Kill,
    /// Report readiness after a startup diagnostic.
    Booting,
    /// Forward invalid and split diagnostic bytes through termination.
    ForwardStderr,
    /// Fill the diagnostic pipe before reporting the native PID.
    DeliverStderr,
    /// Split one diagnostic character across a command barrier.
    SplitStderr,
    /// Emit ticks on acknowledged input.
    Tick,
    /// Report termination without exiting until input closes or the process is killed.
    Resist,
    /// Capture commands while withholding state replies.
    Restart,
    /// Answer each command before emitting the supplied records.
    Burst,
    /// Hold a state reply until the next command.
    LateState,
    /// Announce readiness without answering commands.
    Withhold,
    /// Hold a terminal event until the next command.
    LateEnd,
    /// Answer two commands after both have arrived.
    LateAnswers,
    /// Exit with a diagnostic after two commands.
    LateExit,
    /// Emit a terminal event and acknowledge an answer.
    Answered,
    /// Acknowledge a command without replying.
    Received,
    /// Emit a past terminal event and a future collection.
    Collections,
    /// Emit prompt events except for a quiet prompt.
    Prompt,
    /// Refuse prompts before emitting a terminal event.
    RejectPrompt,
    /// Close standard input while retaining the output pipes.
    ClosedInput,
    /// Answer aborts and exit on a bash command.
    Settle,
    /// Exit while a real descendant retains both output pipes.
    Descendant,
    /// Retain inherited pipes until the test releases the child.
    PipeHolder,
    /// Answer commands with an abort terminal event.
    Cancelled,
    /// Emit the requested number of indexed events.
    Linear,
    /// Refuse every command with its correlated type.
    Refuse,
    /// Replay supplied responses with the current request identifiers.
    Replay,
    /// Answer commands out of order and duplicate the first reply.
    Reversed,
    /// Insert malformed records before a held state reply.
    Malformed,
    /// Emit supplied JSON values interleaved with null records.
    Records,
    /// Split Unicode and JSONL bytes across acknowledged writes.
    Fragments,
}

impl Scenario {
    /// The exact selector written into the installed alias's sibling record.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Session => "Session",
            Self::IdleInterrupt => "IdleInterrupt",
            Self::Launch => "Launch",
            Self::Serve => "Serve",
            Self::Report => "Report",
            Self::Entry => "Entry",
            Self::BootFailure => "BootFailure",
            Self::Kill => "Kill",
            Self::Booting => "Booting",
            Self::ForwardStderr => "ForwardStderr",
            Self::DeliverStderr => "DeliverStderr",
            Self::SplitStderr => "SplitStderr",
            Self::Tick => "Tick",
            Self::Resist => "Resist",
            Self::Restart => "Restart",
            Self::Burst => "Burst",
            Self::LateState => "LateState",
            Self::Withhold => "Withhold",
            Self::LateEnd => "LateEnd",
            Self::LateAnswers => "LateAnswers",
            Self::LateExit => "LateExit",
            Self::Answered => "Answered",
            Self::Received => "Received",
            Self::Collections => "Collections",
            Self::Prompt => "Prompt",
            Self::RejectPrompt => "RejectPrompt",
            Self::ClosedInput => "ClosedInput",
            Self::Settle => "Settle",
            Self::Descendant => "Descendant",
            Self::PipeHolder => "PipeHolder",
            Self::Cancelled => "Cancelled",
            Self::Linear => "Linear",
            Self::Refuse => "Refuse",
            Self::Replay => "Replay",
            Self::Reversed => "Reversed",
            Self::Malformed => "Malformed",
            Self::Records => "Records",
            Self::Fragments => "Fragments",
        }
    }
}

impl FromStr for Scenario {
    /// A selector outside the compiled scenario set.
    type Err = io::Error;

    /// Reject selectors outside the compiled scenario set.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "Session" => Ok(Self::Session),
            "IdleInterrupt" => Ok(Self::IdleInterrupt),
            "Launch" => Ok(Self::Launch),
            "Serve" => Ok(Self::Serve),
            "Report" => Ok(Self::Report),
            "Entry" => Ok(Self::Entry),
            "BootFailure" => Ok(Self::BootFailure),
            "Kill" => Ok(Self::Kill),
            "Booting" => Ok(Self::Booting),
            "ForwardStderr" => Ok(Self::ForwardStderr),
            "DeliverStderr" => Ok(Self::DeliverStderr),
            "SplitStderr" => Ok(Self::SplitStderr),
            "Tick" => Ok(Self::Tick),
            "Resist" => Ok(Self::Resist),
            "Restart" => Ok(Self::Restart),
            "Burst" => Ok(Self::Burst),
            "LateState" => Ok(Self::LateState),
            "Withhold" => Ok(Self::Withhold),
            "LateEnd" => Ok(Self::LateEnd),
            "LateAnswers" => Ok(Self::LateAnswers),
            "LateExit" => Ok(Self::LateExit),
            "Answered" => Ok(Self::Answered),
            "Received" => Ok(Self::Received),
            "Collections" => Ok(Self::Collections),
            "Prompt" => Ok(Self::Prompt),
            "RejectPrompt" => Ok(Self::RejectPrompt),
            "ClosedInput" => Ok(Self::ClosedInput),
            "Settle" => Ok(Self::Settle),
            "Descendant" => Ok(Self::Descendant),
            "PipeHolder" => Ok(Self::PipeHolder),
            "Cancelled" => Ok(Self::Cancelled),
            "Linear" => Ok(Self::Linear),
            "Refuse" => Ok(Self::Refuse),
            "Replay" => Ok(Self::Replay),
            "Reversed" => Ok(Self::Reversed),
            "Malformed" => Ok(Self::Malformed),
            "Records" => Ok(Self::Records),
            "Fragments" => Ok(Self::Fragments),
            _ => Err(io::Error::other("unknown RPC peer scenario")),
        }
    }
}
