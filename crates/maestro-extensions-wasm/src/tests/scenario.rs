//! The scenario both adapters run, and the transcript it must produce.
//!
//! Lines come from three sources in time order: host-side observations (`register`,
//! `new-session`, `wait-for-idle`), the extension's own `entry` calls, and what the driving
//! test received back from each callback.

/// One adapter driven through the scenario. A delivery resolves to the description of what the
/// handler answered, or to the message it failed with, and the scenario writes it to the
/// transcript.
pub trait Driver {
    /// Runs the factory and records the registrations the host observed.
    async fn start(&mut self) -> Result<(), String>;
    /// Delivers an input event.
    async fn input(&mut self, text: &str) -> Result<String, String>;
    /// Delivers a before-compact event whose signal is `aborted` or live.
    async fn compact(&mut self, aborted: bool) -> Result<String, String>;
    /// Delivers an input to the handler that trims it in place, and describes the event as
    /// the handler left it.
    async fn trim_input(&mut self, text: &str) -> Result<String, String>;
    /// Delivers a before-compact event to the handler that notes the tokens in its
    /// preparation, and describes the event as the handler left it.
    async fn note_compaction(&mut self, aborted: bool) -> Result<String, String>;
    /// Runs the command that starts a new session; its continuation waits for idle, and the
    /// wait is held until the driver saw it pending.
    async fn command(&mut self, args: &str) -> Result<(), String>;
    /// Releases every registered callback in registration order, including the ones that
    /// releasing registers.
    async fn release_all(&mut self);
    /// Makes the next session operation fail without running its continuation.
    fn reject_next_session(&mut self);
    /// Invokes a handler and a continuation under an identity the extension never
    /// registered, and returns the errors it reports.
    async fn invoke_unknown_callbacks(&mut self) -> Vec<String>;
    /// Adds a line to the transcript.
    fn log(&mut self, line: String);
    /// The lines observed so far.
    fn transcript(&self) -> Vec<String>;
    /// The identities the extension dropped, in drop order.
    fn dropped(&self) -> Vec<u32>;
    /// How many resources the host handed to the extension that it has not dropped.
    fn live(&self) -> usize;
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
