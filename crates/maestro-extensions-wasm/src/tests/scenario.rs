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

/// Resources and values requested for one tool invocation.
pub struct ToolRun<'a> {
    /// Registered tool name.
    pub name: &'a str,
    /// Invocation identity.
    pub id: &'a str,
    /// JSON parameters.
    pub params: &'a str,
    /// Optional lent cancellation handle.
    pub signal: Option<usize>,
    /// Whether progress is supplied.
    pub update: bool,
    /// Holds execution until its wait is observed, cancelling the supplied signal.
    pub held: bool,
}

/// Ordered command and shortcut registrations recorded at the host boundary.
pub type Registrations = (
    Vec<(String, Option<String>, bool)>,
    Vec<(String, Option<String>)>,
);

/// One adapter driven through the scenario. A delivery resolves to the description of what the
/// handler answered, or to the message it failed with, and the scenario writes it to the
/// transcript.
pub trait Driver {
    /// The operation resource returned by the most recent delivery.
    fn returned_bash(&self) -> Option<usize>;
    /// Creates an independently observed byte receiver.
    fn shell_output(
        &mut self,
        error: Option<String>,
        cancel: Option<usize>,
    ) -> Result<usize, String>;
    /// Invokes an actual returned operation with typed operands.
    async fn shell_exec(&mut self, run: super::shell_host::Run) -> Result<Option<f64>, String>;
    /// Drops one returned operation alias.
    async fn drop_bash(&mut self, id: usize) -> Result<(), String>;
    /// Successfully received chunks observed so far.
    fn shell_chunks(&self, id: usize) -> Vec<Vec<u8>>;
    /// Signal final-owner count for the supplied handle.
    fn shell_signal_drops(&self, id: usize) -> usize;
    /// Output final-owner count.
    fn shell_drops(&self, id: usize) -> usize;

    /// Actual invocation-local emitter identities and attempted payloads.
    fn renderer_emissions(&self) -> Vec<(usize, String, serde_json::Value)>;
    /// Invokes a renderer with an independently owned theme.
    async fn renderer(
        &mut self,
        handler: u32,
        message: &str,
        expanded: bool,
        theme: &str,
    ) -> Result<Option<usize>, String>;
    /// Invokes a retained component method.
    async fn component(
        &mut self,
        id: usize,
        call: super::renderer_host::Call<'_>,
    ) -> Result<super::renderer_host::Reply, String>;
    /// Removes one returned component alias.
    async fn drop_component(&mut self, id: usize) -> Result<(), String>;
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
    /// Repeats release through a fresh borrow of an existing identity.
    async fn repeat_release(&mut self, handler: u32) -> Result<(), String>;
    /// Runs one tool renderer for the row its info names, keeping the row's guest state, and
    /// renders the returned component at `width`.
    async fn render_tool(
        &mut self,
        run: ToolRenderRun<'_>,
        width: u32,
    ) -> Result<Vec<String>, String>;
    /// Renders the custom footer, if the host shows one.
    async fn footer(&mut self, width: u32) -> Result<Option<Vec<String>>, String>;
    /// Runs every live branch change listener in registration order and returns each answer.
    async fn branch_change(&mut self) -> Result<Vec<serde_json::Value>, String>;
    /// The header container's children and the rendering of its custom header, if any.
    async fn header(&mut self, width: u32) -> Result<(Vec<String>, Option<Vec<String>>), String>;
    /// Renders the factory widget the host shows under `key`, or reports that it shows none.
    async fn widget(&mut self, key: &str, width: u32) -> Result<Vec<String>, String>;
    /// Reads a factory component's width and calls its expansion handler when present.
    async fn widget_capabilities(
        &mut self,
        key: &str,
        expanded: bool,
    ) -> Result<(Option<f64>, bool), String>;
    /// Runs one host request on the authored editor the host shows, describing the answer as
    /// [`super::editor_host`] does.
    async fn editor_request(
        &mut self,
        request: serde_json::Value,
    ) -> Result<serde_json::Value, String>;
    /// Runs one host request on the authored completion provider on top of the host's stack,
    /// describing the answer as [`super::provider_host`] does.
    async fn provider_request(
        &mut self,
        request: serde_json::Value,
    ) -> Result<serde_json::Value, String>;
    /// Calls one method of the component of the authored editor the host shows.
    async fn editor_component(
        &mut self,
        call: super::renderer_host::Call<'_>,
    ) -> Result<super::renderer_host::Reply, String>;
    /// Delivers one terminal input chunk to every live terminal input listener in
    /// registration order and returns each listener's answer.
    async fn terminal_input(&mut self, data: &str) -> Result<Vec<serde_json::Value>, String>;
    /// Delivers one compaction notification through an existing borrowed emitter.
    async fn notify_compaction(
        &mut self,
        handler: u32,
        payload: &str,
        complete: bool,
    ) -> Result<(), String>;
    /// Synchronously invokes a registered preparation callback.
    async fn prepare(&mut self, name: &str, args: &str) -> Result<String, String> {
        self.prepare_key(self.identity(&format!("prepare {name}"))?, args)
            .await
    }
    /// Invokes preparation through a supplied identity.
    async fn prepare_key(&mut self, handler: u32, args: &str) -> Result<String, String>;
    /// Lends a callback identity with the specified logical key.
    fn reborrow(&mut self, id: u32) -> Result<u32, String>;
    /// Awaits a tool callback with selected resources.
    async fn tool(&mut self, run: ToolRun<'_>) -> Result<String, String>;
    /// Registered tool metadata.
    fn tools(&self) -> Vec<String>;
    /// Sets a controlled host emission rejection.
    fn reject_bus_emission(&mut self, error: Option<String>);
    /// Event-channel payloads observed at the host boundary.
    fn bus_emissions(&self) -> Vec<(String, serde_json::Value)>;
    /// Partial outputs received by this host.
    fn updates(&self) -> Vec<String>;
    /// Invokes a completion identity, preserving the optional ordered candidate list.
    async fn completion(
        &mut self,
        handler: u32,
        prefix: &str,
    ) -> Result<Option<Vec<serde_json::Value>>, String>;
    /// Completes with a named wait-for-idle barrier observed before it opens.
    async fn completion_held(
        &mut self,
        handler: u32,
        prefix: &str,
    ) -> Result<Option<Vec<serde_json::Value>>, String>;
    /// Invokes a shortcut with a distinctly bound ordinary context.
    async fn shortcut(&mut self, handler: u32, cwd: &str) -> Result<(), String>;
    /// Command and shortcut registration observations in order.
    fn registrations(&self) -> Registrations;
    /// Invoke setup through a lent callback identity.
    async fn notify_setup(&mut self, handler: u32) -> Result<(), String>;
    /// Drop the operation while its admitted setup is pending, then await its own final owner.
    async fn setup_drop(&mut self) -> Result<(), String> {
        self.drop_command("setup", &super::setup_sessions::drop_input())
            .await
    }
    /// Await the capture teardown of a command that drops an admitted callback operation.
    async fn drop_command(&mut self, name: &str, args: &str) -> Result<(), String>;
    /// Invoke a continuation under a supplied identity.
    async fn notify_continuation(&mut self, handler: u32) -> Result<(), String>;
    /// Drop an operation while its host admission gate remains closed and await its teardown.
    async fn host_drop(&mut self, args: &str) -> Result<(), String>;
    /// Overlap two writable setup operations and finish the second first.
    async fn setup_pair(&mut self) -> Result<(), String>;
    /// Complete switch and fork before the older new-session invocation.
    async fn replacement_triple(&mut self) -> Result<(), String>;
    /// Finish B's send without releasing A's independent send gate.
    async fn replacement_sends_pair(&mut self) -> Result<(), String>;
    /// Runs two calls with distinct inputs through named entry/completion barriers.
    async fn pending_pair(&mut self, handler: u32, shortcut: bool) -> Result<(), String>;
    /// The exact missing/wrong-kind diagnostic for a lent callback resource.
    fn callback_error(&self, handler: u32) -> String;
    /// Runs a command with its first host barrier held until witnessed.
    async fn command_held(&mut self, name: &str, args: &str) -> Result<(), String>;
    /// Runs a shortcut with its first host barrier held until witnessed.
    async fn shortcut_held(&mut self, handler: u32, cwd: &str) -> Result<(), String>;
    /// Configures a recoverable registration-import error.
    fn registration_error(&mut self, error: Option<String>);
    /// Installs a supplied flag reply without performing default policy.
    fn flag_reply(&mut self, name: &str, value: Result<Option<serde_json::Value>, String>);
    /// Flag registrations observed at the host boundary.
    fn flag_registrations(&self) -> Vec<(String, serde_json::Value)>;
    /// Runs the command registered as `name` against a command context in `/work`.
    async fn run_plain_command(&mut self, name: &str, args: &str) -> Result<(), String>;
    /// Runs the command that starts a new session; its continuation waits for idle, and the
    /// wait is held until the driver saw it pending.
    async fn command(&mut self, args: &str) -> Result<(), String>;
    /// Drops every retained component, then releases every registered callback in
    /// registration order, including the ones that releasing registers.
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

/// Reads the answer an event route returned as its input transformation.
pub fn transformed(decision: Decision) -> Result<serde_json::Value, String> {
    let Decision::Returned(Ok(Some(text))) = decision else {
        return Err("missing event answer".into());
    };
    let transform: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| error.to_string())?;
    assert_eq!(transform["action"], "transform");
    serde_json::from_str(transform["text"].as_str().ok_or("missing text")?)
        .map_err(|error| error.to_string())
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

/// One tool rendering: the tool, the renderer slot, what the renderer reads and, for a result,
/// the result document.
pub struct ToolRenderRun<'a> {
    /// Tool name.
    pub tool: &'a str,
    /// `call` or `result`.
    pub slot: &'a str,
    /// The row's call id, arguments, directory and flags.
    pub info: &'a serde_json::Value,
    /// The result document of a result rendering.
    pub result: Option<&'a serde_json::Value>,
}
