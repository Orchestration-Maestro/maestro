# Shared assistant output

[`initial_message`](../../crates/maestro-models/src/providers/assistant_output.rs)
creates an empty assistant message with the supplied model's API, provider and ID,
zero usage, a stop reason of `stop` and the current timestamp. Protocol callers
retain responsibility for any protocol-specific identity overrides.

[`publish_failure`](../../crates/maestro-models/src/providers/assistant_output.rs)
updates the retained message's stop reason and error text before publishing its
handle, releasing the message lock before notifying readers. An aborted signal
selects `aborted`; otherwise it selects `error`. Formatting and explicit producer
settlement remain with callers. A settled stream ignores another event, but the
retained message still receives the failure state.
