//! The scenario both adapters run, and the transcript it must produce.
//!
//! Lines come from three sources in time order: host-side observations (`register`,
//! `new-session`, `user-message`), the extension's own `entry` calls, and what the driving
//! test received back from each callback.
/// One adapter driven through the scenario.
pub trait Driver {
    /// Runs the factory and records the registrations the host observed.
    async fn start(&mut self) -> Result<(), String>;
    /// Prepares the arguments of the echo tool and runs it with a live signal.
    async fn tool(&mut self, call_id: &str);
    /// Cancels the signal of the previous tool call, which the extension retained.
    fn abort_tool_signal(&mut self);
    /// Delivers an input event.
    async fn input(&mut self, text: &str);
    /// Delivers a bash tool call whose command the handler edits and then fails on.
    async fn tool_call(&mut self, command: &str);
    /// Delivers a before-compact event whose signal is `aborted` or live.
    async fn compact(&mut self, aborted: bool);
    /// Runs the command that starts a new session, held until the driver saw it pending.
    async fn command(&mut self, args: &str);
    /// Releases every registered callback in registration order.
    async fn release_all(&mut self);
    /// The lines observed so far.
    fn transcript(&self) -> Vec<String>;
}

/// Runs the scenario and returns the transcript.
///
/// # Errors
/// Returns the message the factory failed with.
pub async fn run(driver: &mut impl Driver) -> Result<Vec<String>, String> {
    driver.start().await?;
    driver.input("hello").await;
    driver.tool_call("ls").await;
    driver.compact(false).await;
    driver.compact(true).await;
    driver.tool("c1").await;
    driver.abort_tool_signal();
    driver.tool("c2").await;
    driver.command("go").await;
    driver.release_all().await;
    Ok(driver.transcript())
}

/// Expected transcript of the full scenario.
pub const EXPECTED: &[&str] = &[
    "register event input",
    "register event tool_call",
    "register event session_before_compact",
    "register tool echo prepare=true",
    "register command replace",
    "reject event rejected",
    r#"entry released "rejected-handler""#,
    r#"entry rejection "registration rejected: rejected""#,
    "register event reentrant",
    r#"entry input {"text":"hello","cwd":"/work"}"#,
    "input hello -> transform:HELLO",
    "tool_call ls -> command=ls -la error=tool call blocked",
    "compact aborted=false -> cancel=false summary=previous_aborted:None",
    "compact aborted=true -> cancel=true summary=previous_aborted:Some(true)",
    r#"emit tool:prepared {"text":"x","prepared":true}"#,
    r#"prepared {"text":"x","prepared":true}"#,
    "tool-update c1 working",
    r#"tool-result c1 content=echo x/sig-é details={"call":"c1","previous_aborted":null} terminate=true"#,
    r#"emit tool:prepared {"text":"x","prepared":true}"#,
    r#"prepared {"text":"x","prepared":true}"#,
    "tool-update c2 working",
    r#"tool-result c2 content=echo x/sig-é details={"call":"c2","previous_aborted":true} terminate=true"#,
    r#"entry command {"args":"go","cwd":"/work"}"#,
    "new-session start parent=parent",
    r#"entry continuation {"cwd":"/replacement"}"#,
    "user-message /replacement hello from continuation",
    r#"entry released "continuation""#,
    "new-session done cancelled=false",
    r#"entry stale "This context is stale after session replacement.""#,
    r#"entry after {"before":"/work","cancelled":false}"#,
    r#"entry released "input-handler""#,
    r#"entry released "tool-call-handler""#,
    r#"entry released "compaction-handler""#,
    r#"entry released "tool-prepare""#,
    r#"entry released "tool-execute""#,
    r#"entry released "command-handler""#,
    r#"entry released "reentrant-guard""#,
    "register event late",
];
