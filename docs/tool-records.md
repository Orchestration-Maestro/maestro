# Tool interchange records

The `tool` module of `maestro-interchange`, at `maestro_interchange::tool`, owns the shared
declarations for `ToolExecutionMode`, `AgentToolResult`, `ReadToolDetails`,
`TruncationBy`, `TruncationResult` and `BashResult`. It contains no execution or
selection policy. For results using `Option` fields,
`AgentToolResult::text_result(text, details)` builds one text block with the
supplied details and no termination request.

Its `types` module also declares the eight records the native tools and guest authoring
both use: `BashExecResult`, `ExecResult`, `FindGlobOptions`, `ToolName`, `ToolRenderPhase`,
`ToolRenderResultOptions`, `TruncateLineResult` and `TruncationOptions`.
Both sides re-export them at their existing paths. `ToolName` parses from and displays as
its lowercase tool name, and an unknown name is an error. `TruncateLineResult` serializes
with camel-case members; the guest reads `ExecResult` from JSON.

The existing native and guest result paths are aliases or re-exports. Native
truncation counts use `usize`; guest counts use `f64`. Optional fields retain
native `Option` or guest `Presence` representations, with the shared
[`Omittable`](../crates/maestro-interchange/src/presence.rs) predicate selecting omitted
fields. [Request records](records.md) describe presence; the
[JSON boundary](json.md) owns JSON operations.

Read-input declarations remain in their native and guest owners, retaining
their separate admission policies. [Native tools](tools-read.md) own truncation
selection and read execution; [guest authoring](extensions/guest.md) owns the
extension interfaces that exchange these records.
