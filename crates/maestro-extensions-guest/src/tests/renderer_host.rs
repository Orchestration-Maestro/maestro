//! Shared renderer transport requests and resource observations.
/// Independently owned host theme identity.
pub struct Theme(pub String);
/// Requested operation on a retained component.
#[derive(Clone, Copy)]
pub enum Call<'a> {
    /// Render at a caller-selected width.
    Render(u32),
    /// Forward literal input.
    Input(&'a str),
    /// Read the live release preference.
    Release,
    /// Invalidate the retained view.
    Invalidate,
}
/// Typed observations returned by component methods.
#[derive(Debug, PartialEq)]
pub enum Reply {
    /// Ordered rendering lines.
    Lines(Vec<String>),
    /// Release preference.
    Release(bool),
    /// Completed void method.
    Done,
}
/// Invokes a component facade using the same borrowed-emitter adapter as generated methods.
pub(super) fn invoke(
    component: &crate::Component,
    call: Call<'_>,
    host: &super::controlled::Controlled,
    key: usize,
) -> Result<Reply, String> {
    use crate::component_adapter::renderers::with_emitter;
    with_emitter(
        host,
        &super::bus_host::Emitter { key },
        |emitter| match call {
            Call::Render(width) => component.render(width, emitter).map(Reply::Lines),
            Call::Input(data) => component.handle_input(data, emitter).map(|()| Reply::Done),
            Call::Release => component.wants_key_release(emitter).map(Reply::Release),
            Call::Invalidate => component.invalidate(emitter).map(|()| Reply::Done),
        },
    )
}

use serde::Deserialize;
use serde_json::Value;
/// Recorded source branch.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub(super) enum Group {
    /// Complete message/options forwarding.
    Message,
    /// Absent, present or returned-error branch.
    Outcome,
    /// Literal registration and replacement.
    Registration,
    /// Rejected registration preserving its predecessor.
    RejectedRegistration,
    /// Stateful render/input/invalidation.
    Component,
    /// Live optional release preference.
    ReleasePreference,
    /// Exact authored line vector.
    LineSequence,
    /// Default input avoids rendering.
    AbsentInput,
    /// Optional shared text signature.
    SignedText,
    /// Finite timestamp and signed zero.
    Timestamp,
}
/// One strict source query; each branch selects its own input and expectation types.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Case {
    /// Source branch used for dispatch.
    pub group: Group,
    /// Branch-specific authored input.
    pub input: Value,
    /// Independently recorded result.
    pub expected: Value,
}
/// Native decoder observation through a public renderer operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Decoder {
    /// Case attribution.
    pub name: String,
    /// Unchanged raw document, including duplicate members.
    pub raw: String,
    /// Pinned decoder result.
    pub native: Native,
}
/// Native accepted record or exact diagnostic.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub(super) enum Native {
    /// Accepted serialized record and floating-point sign.
    Ok {
        /// Surviving shared record.
        ok: Value,
        /// Sign bit of a zero timestamp.
        negative_zero: bool,
    },
    /// Rejected selected field.
    Error {
        /// Unmodified native error.
        error: String,
    },
}
/// Source message invocation operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MessageInput {
    /// Complete source message before opaque details encoding.
    pub message: Value,
    /// Caller-selected expansion.
    pub expanded: bool,
}
/// Recorded message invocation result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MessageExpected {
    /// Complete received source message.
    pub message: Value,
    /// Expansion observed by the callback.
    pub expanded: bool,
    /// Independently supplied theme owner.
    pub theme: String,
    /// Whether a component was returned.
    pub returned: bool,
}
/// Selected renderer return branch.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OutcomeInput {
    /// None, component or author failure.
    pub mode: String,
    /// Expansion supplied to this invocation.
    pub expanded: bool,
}
/// Recorded optional result or failure.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OutcomeExpected {
    /// Optional component presence.
    pub returned: Option<bool>,
    /// Optional exact failure.
    pub error: Option<String>,
    /// Callback invocation count.
    pub called: usize,
}
/// Literal registration operand.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RegistrationInput {
    /// Literal name.
    pub name: String,
}
/// Source registration replacement and selected callback.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RegistrationExpected {
    /// Ordered distinct registration names.
    pub keys: Vec<String>,
    /// Authored marker of the replacement callback.
    pub selected: u32,
    /// Whether an unregistered identity is absent.
    pub missing: bool,
}
/// Rejection preserves the previous callback.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RejectedExpected {
    /// Host rejection text.
    pub error: String,
    /// Marker of the surviving callback.
    pub selected: u32,
}
/// Stateful component operation input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ComponentInput {
    /// Literal viewport width.
    pub width: u32,
    /// Literal input data.
    pub data: String,
}
/// Ordered lines before and after edit/invalidation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ComponentExpected {
    /// Initial rendering.
    pub before: Vec<String>,
    /// Rendering of the same view after its state changes.
    pub after: Vec<String>,
}
/// Optional live release preference.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReleaseInput {
    /// Missing preference represents the default.
    pub preference: Option<bool>,
}
/// Recorded release decision before and after mutation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReleaseExpected {
    /// Source consumer filters release before the edit.
    pub filter_release_before: bool,
    /// Source consumer filters release after the edit.
    pub filter_release_after: bool,
}
/// Exact authored line sequence.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Lines {
    /// Ordered lines, including repetitions and blanks.
    pub lines: Vec<String>,
}
/// Literal input sent to the default handler.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AbsentInput {
    /// Input data.
    pub data: String,
}
/// Source avoided-render observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AbsentExpected {
    /// Synchronous renders during input dispatch.
    pub renders: usize,
}
/// Complete shared text record with a signature.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Signed {
    /// Message containing the selected signature.
    pub message: Value,
}
/// Exact finite numeric syntax, retaining signed zero separately from JSON values.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Timestamp {
    /// Source numeric syntax.
    pub literal: String,
}
