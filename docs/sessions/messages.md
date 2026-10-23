# Conversation message projection

`maestro-session::messages` formats recorded shell executions, constructs
application messages and projects them into model-request messages.
`maestro-session::convert_to_llm` is also available at the crate root.

The converter preserves the input sequence, copying base model records into the
owned request. Injected custom content becomes user input regardless of display
visibility. Shell executions marked `exclude_from_context: Present(true)` and
unknown caller variants are omitted. Branch and compaction summaries use their
respective literal wrappers. Returned requests are snapshots; a later conversion
reads the retained input handles again.

`bash_execution_to_text` preserves authored command/output text. Empty output gets
`(no output)`, cancellation takes precedence over nonzero exit status, and a
truncation notice requires both the flag and a nonempty full-output path.
Suppression belongs to the converter, not this formatter.

The constructors retain supplied fields and parse timestamps into epoch
milliseconds using chrono: explicit-offset ISO instants, calendar dates at UTC
midnight, or offset-less local date-times. Invalid input becomes NaN, serialized
as null. Submillisecond precision is discarded. Legacy free-form date parsing
is not provided.

`maestro_session::messages::parse_timestamp_millis`, also exposed through
`maestro_app`, returns `Option<i64>` for the same date forms. Ambiguous local
fall-back times select the earlier instant. A local time a daylight-saving change
skips resolves to the instant the offset in force at that reading gives, so it
advances by the change the zone actually made instead of being rejected. Invalid
input returns `None`; the message constructors retain their NaN fallback.

`maestro_session::messages::local_timestamp_millis` is that same resolution on its
own, for callers that already hold a local date-time. Reports use it for calendar
boundaries, so no second local-time resolver exists.

The open `CustomAgentMessages<T, C>` enum bridges these four application variants
to the agent's custom-role trait. `Other(C)` retains caller-defined roles without
projecting them. Without caller-defined roles, the enum reads and writes stored
message records by their `role`; an unknown role is an error. Install a closure
invoking the converter in
`maestro_agent::AgentOptions::convert_to_llm` to apply this projection at the
model-request boundary.

See [shared application records](../records.md#application-message-records) for
record ownership and wire representation, and [session libraries](../libraries/sessions.md)
for the library choices. History management and persistence are separate APIs.
