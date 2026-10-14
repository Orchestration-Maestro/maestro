# Tool interchange records

`maestro-tool` owns the shared declarations for `ToolExecutionMode`,
`AgentToolResult`, `ReadToolDetails`, `TruncationBy`, `TruncationResult` and
`BashResult`. It depends only on `maestro-request` within the workspace and
contains no execution or selection policy.

The existing native and guest result paths are aliases or re-exports. Native
truncation counts use `usize`; guest counts use `f64`. Optional fields retain
native `Option` or guest `Presence` representations, with the shared
[`Omittable`](../crates/maestro-request/src/presence.rs) predicate selecting omitted
fields. [Request records](records.md) describe presence; the
[JSON boundary](json.md) owns JSON operations.

Read-input declarations remain in their native and guest owners, retaining
their separate admission policies. [Native tools](tools-read.md) own truncation
selection and read execution; [guest authoring](extensions/guest.md) owns the
extension interfaces that exchange these records.
