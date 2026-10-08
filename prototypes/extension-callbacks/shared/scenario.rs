//! The transcript both drivers must produce for the example extension.
//!
//! Lines come from three sources in time order: host-side observations (`register`,
//! `tool-update`, `user-message`, `new-session`), the extension's own `entry` calls,
//! and what the driving test received back from each callback.

/// Expected transcript of the full scenario.
pub const EXPECTED: &[&str] = &[
    "register input",
    "register tool echo prepare=true",
    "register command replace",
    "register renderer note",
    r#"entry input {"text":"hello","cwd":"/work"}"#,
    "input-outcome text=hello decision=transform:HELLO",
    r#"entry input {"text":"reject","cwd":"/work"}"#,
    "input-outcome text=reject! decision=error:input rejected",
    r#"prepared {"text":"x","prepared":true}"#,
    "tool-update c1 working",
    r#"tool-result c1 content=echo x details={"call":"c1","previous_aborted":null}"#,
    "tool-update c2 working",
    r#"tool-result c2 content=echo x details={"call":"c2","previous_aborted":true}"#,
    r#"entry command {"args":"go","cwd":"/work"}"#,
    "new-session start parent=parent",
    r#"entry continuation {"cwd":"/replacement"}"#,
    "user-message /replacement hello from continuation",
    r#"entry released "continuation""#,
    "new-session done cancelled=false",
    r#"entry stale "This context is stale after session replacement.""#,
    r#"entry after {"before":"/work","cancelled":false}"#,
    "rendered 20|body|expanded",
    r#"entry invalidated "note""#,
    r#"entry released "note-view""#,
    r#"entry released "input-handler""#,
    r#"entry released "tool-execute""#,
    r#"entry released "command-handler""#,
    r#"entry released "renderer""#,
];
