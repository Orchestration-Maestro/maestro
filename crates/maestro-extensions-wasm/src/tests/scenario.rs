//! The scenario both adapters run, and the transcript it must produce.
//!
//! Lines come from three sources in time order: host-side observations (`register`,
//! `new-session`, `wait-for-idle`), the extension's own `entry` calls, and what the driving
//! test received back from each callback.

use super::fixtures;

/// Encoded text, no text, or the message the encoding failed with.
pub type Encoded = Result<Option<String>, String>;

/// How an event callback ended, independent of the bindings that carried it.
#[derive(Debug, PartialEq)]
pub enum Decision {
    /// The callback returned; its optional result, encoded.
    Returned(Encoded),
    /// The callback failed with this message.
    Failed(String),
}

/// An event after its callback ran.
#[derive(Debug, PartialEq)]
pub struct Delivery {
    /// The event as the callback left it.
    pub event: Encoded,
    /// What the callback decided.
    pub decision: Decision,
}

/// The identity of a handler no extension registered.
pub const UNKNOWN: u32 = 4242;

/// One adapter driven through the scenario. A delivery resolves to the description of what the
/// handler answered, or to the message it failed with, and the scenario writes it to the
/// transcript.
pub trait Driver {
    /// Runs the factory and records the registrations the host observed.
    async fn start(&mut self) -> Result<(), String>;
    /// The identity a handler was registered under, such as `event input`.
    ///
    /// # Errors
    /// Returns the message when nothing is registered under the name.
    fn identity(&self, name: &str) -> Result<u32, String>;
    /// Delivers the JSON document `event` to the handler under `handler`, with a context in
    /// `cwd` and the lent signal `signal`; the callback owns both resources. Resolves to the
    /// outcome, or to the message the document or its resources were rejected with before
    /// the handler ran.
    async fn deliver(
        &mut self,
        handler: u32,
        event: &str,
        cwd: &str,
        signal: Option<usize>,
    ) -> Result<Delivery, String>;
    /// Like [`Driver::deliver`], with the host's next wait for idle held pending until the
    /// driver saw it, which it records as an observation.
    async fn deliver_held(
        &mut self,
        handler: u32,
        event: &str,
        cwd: &str,
    ) -> Result<Delivery, String>;
    /// A handler key the extension never registered; the extension reports it as identity
    /// [`UNKNOWN`].
    ///
    /// # Errors
    /// Returns the message when the host cannot lend one.
    fn unregistered(&mut self) -> Result<u32, String>;
    /// Lends the host's cancellation flag, already cancelled when `aborted`, and returns its
    /// handle.
    ///
    /// # Errors
    /// Returns the message when the host cannot lend it.
    fn lend_signal(&mut self, aborted: bool) -> Result<usize, String>;
    /// Cancels a lent flag, including one the extension retained.
    ///
    /// # Errors
    /// Returns the message when the host no longer has the flag.
    fn abort_signal(&mut self, signal: usize) -> Result<(), String>;
    /// The handle of the flag lent for the most recent compaction.
    fn previous_signal(&mut self) -> &mut Option<usize>;
    /// Runs the command registered as `name` against a command context in `/work`.
    async fn run_plain_command(&mut self, name: &str, args: &str) -> Result<(), String>;
    /// Runs the command that starts a new session; its continuation waits for idle, and the
    /// wait is held until the driver saw it pending.
    async fn command(&mut self, args: &str) -> Result<(), String>;
    /// Releases every registered callback in registration order, including the ones that
    /// releasing registers.
    async fn release_all(&mut self);
    /// Releases the callback registered as `name`; whether there was one.
    async fn release(&mut self, name: &str) -> bool;
    /// Makes the next session operation fail without running its continuation.
    fn reject_next_session(&mut self);
    /// Invokes a continuation under an identity the extension never registered, and returns
    /// the error it reports.
    async fn invoke_unknown_continuation(&mut self) -> String;
    /// Adds a line to the transcript.
    fn log(&mut self, line: String);
    /// The lines observed so far.
    fn transcript(&self) -> Vec<String>;
    /// The identities the extension dropped, in drop order.
    fn dropped(&self) -> Vec<u32>;
    /// How many resources the host handed to the extension that it has not dropped.
    fn live(&self) -> usize;

    /// Delivers an interactive input to the handler registered as `name`.
    async fn deliver_input(&mut self, name: &str, text: &str) -> Result<Delivery, String> {
        let handler = self.identity(name)?;
        let event = serde_json::json!({ "type": "input", "text": text, "source": "interactive" });
        self.deliver(handler, &event.to_string(), "/work", None)
            .await
    }

    /// Delivers a before-compact event with the lent flag `signal` to the handler registered
    /// as `name`.
    async fn deliver_compact(&mut self, name: &str, signal: usize) -> Result<Delivery, String> {
        let handler = self.identity(name)?;
        let event = serde_json::json!({
            "type": "session_before_compact",
            "branchEntries": [],
            "preparation": { "firstKeptEntryId": "e2", "isSplitTurn": false, "tokensBefore": 1234.0, "messagesToSummarize": [], "turnPrefixMessages": [], "fileOps": {"read":[],"written":[],"edited":[]}, "settings":{"enabled":true,"reserveTokens":1.0,"keepRecentTokens":2.0} },
        });
        self.deliver(handler, &event.to_string(), "/work", Some(signal))
            .await
    }

    /// Delivers an input event.
    async fn input(&mut self, text: &str) -> Result<String, String> {
        let delivery = self.deliver_input("event input", text).await?;
        Ok(fixtures::input_decision(&delivery))
    }

    /// Delivers a before-compact event whose signal is `aborted` or live. The handler keeps
    /// the signal; cancelling the signal of the previous compaction first lets the handler
    /// report the state of the one it kept.
    async fn compact(&mut self, aborted: bool) -> Result<String, String> {
        if aborted && let Some(previous) = *self.previous_signal() {
            self.abort_signal(previous)?;
        }
        let signal = self.lend_signal(aborted)?;
        *self.previous_signal() = Some(signal);
        let delivery = self
            .deliver_compact("event session_before_compact", signal)
            .await?;
        Ok(fixtures::compact_decision(&delivery))
    }

    /// Delivers an input to the handler that trims it in place, and describes the event as
    /// the handler left it.
    async fn trim_input(&mut self, text: &str) -> Result<String, String> {
        let delivery = self.deliver_input("event trim_input", text).await?;
        Ok(fixtures::trimmed(&delivery))
    }

    /// Delivers a before-compact event to the handler that notes the tokens in its
    /// preparation, and describes the event as the handler left it.
    async fn note_compaction(&mut self, aborted: bool) -> Result<String, String> {
        let signal = self.lend_signal(aborted)?;
        let delivery = self
            .deliver_compact("event note_compaction", signal)
            .await?;
        Ok(fixtures::noted(&delivery))
    }
}

/// Writes the outcome of a delivery to the transcript as `label -> description`, or as
/// `label failed: message`.
fn report(driver: &mut impl Driver, label: &str, outcome: Result<String, String>) {
    driver.log(match outcome {
        Ok(description) => format!("{label} -> {description}"),
        Err(message) => format!("{label} failed: {message}"),
    });
}

/// Runs the command and writes its failure, if any, to the transcript.
pub async fn command(driver: &mut impl Driver, args: &str) {
    if let Err(message) = driver.command(args).await {
        driver.log(format!("command failed: {message}"));
    }
}

/// Runs the scenario and returns the transcript.
///
/// # Errors
/// Returns the message the factory failed with.
pub async fn run(driver: &mut impl Driver) -> Result<Vec<String>, String> {
    driver.start().await?;
    let outcome = driver.input("hello").await;
    report(driver, "input hello", outcome);
    for aborted in [false, true] {
        let outcome = driver.compact(aborted).await;
        report(driver, &format!("compact aborted={aborted}"), outcome);
    }
    for text in ["  hi  ", "   "] {
        let outcome = driver.trim_input(text).await;
        report(driver, &format!("trim {text:?}"), outcome);
    }
    for aborted in [false, true] {
        let outcome = driver.note_compaction(aborted).await;
        report(driver, &format!("note aborted={aborted}"), outcome);
    }
    command(driver, "go").await;
    driver.release_all().await;
    Ok(driver.transcript())
}

/// Expected transcript of the full scenario.
pub const EXPECTED: &[&str] = &[
    "register event input",
    "register event session_before_compact",
    "register command replace",
    "reject event rejected",
    r#"entry released "rejected-handler""#,
    r#"entry rejection "registration rejected: rejected""#,
    "register event reentrant",
    "register event trim_input",
    "register event note_compaction",
    "register event probe",
    "register command capture",
    r#"entry input {"text":"hello","cwd":"/work"}"#,
    "input hello -> transform:HELLO",
    "compact aborted=false -> cancel=false summary=previous_aborted:None",
    "compact aborted=true -> cancel=true summary=previous_aborted:Some(true)",
    r#"trim "  hi  " -> ok edited=Some("hi")"#,
    r#"trim "   " -> error:nothing is left of the input after trimming edited=Some("")"#,
    r#"note aborted=false -> ok edited=Some(Some("noted 1234 tokens"))"#,
    r#"note aborted=true -> error:compaction aborted after the note edited=Some(Some("noted 1234 tokens"))"#,
    r#"entry command {"args":"go","cwd":"/work"}"#,
    "new-session start parent=parent",
    r#"entry continuation {"cwd":"/replacement"}"#,
    "wait-for-idle /replacement",
    "command suspended after wait-for-idle /replacement",
    r#"entry released "continuation""#,
    "new-session done cancelled=false",
    r#"entry stale "This context is stale after session replacement.""#,
    r#"entry after {"before":"/work","cancelled":false}"#,
    r#"entry released "input-handler""#,
    r#"entry released "compaction-handler""#,
    r#"entry released "command-handler""#,
    r#"entry released "reentrant-guard""#,
    "register event late",
];
