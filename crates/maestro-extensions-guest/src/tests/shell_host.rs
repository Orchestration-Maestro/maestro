//! Host byte receivers and independently retained execution operands.
use serde::Deserialize;
use serde_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
/// One typed execution request shared by both drivers.
pub struct Run {
    /// Resource slot to invoke.
    pub operation: usize,
    /// Literal command.
    pub command: String,
    /// Literal working directory.
    pub cwd: String,
    /// Unconverted optional seconds.
    pub timeout: Option<f64>,
    /// Optional environment pairs, including missing values.
    pub env: Option<Vec<(String, Option<String>)>>,
    /// Optional independently controlled flag.
    pub signal: Option<usize>,
    /// Receiver identity.
    pub output: usize,
    /// Registration to release while execution is suspended.
    pub release_pending: Option<u32>,
}
/// Host-controlled byte receiver policy.
#[derive(Default)]
pub struct Output {
    /// Successful ordered chunks.
    pub chunks: Vec<Vec<u8>>,
    /// Error returned on the second attempted chunk.
    pub error: Option<String>,
    /// Flag cancelled after the first delivered chunk.
    pub cancel: Option<Arc<AtomicBool>>,
    /// Attempts including rejected chunks.
    pub(super) attempts: usize,
    /// Closed-owner observation.
    pub drops: usize,
}
impl Output {
    /// Delivers one raw chunk without combining or decoding it.
    pub fn data(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.attempts += 1;
        if self.attempts == 2
            && let Some(error) = &self.error
        {
            return Err(error.clone());
        }
        self.chunks.push(bytes.to_vec());
        if self.attempts == 1
            && let Some(flag) = &self.cancel
        {
            flag.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}
/// Output receiver table; no callback registrations are stored here.
#[derive(Default)]
pub struct Bank {
    /// Distinguishable output receiver identities.
    pub outputs: Vec<Output>,
    /// Final drops of each separately keyed signal resource.
    pub signal_drops: std::collections::BTreeMap<usize, usize>,
}
/// Host resource retaining one output identity.
pub struct Receiver(pub usize);
/// Native host resource retaining one output identity.
pub struct NativeReceiver {
    /// Shared observations.
    pub observed: std::rc::Rc<std::cell::RefCell<super::observed::Observed>>,
    /// Receiver identity.
    pub id: usize,
    /// Resource lifetime witness.
    pub(super) _lease: super::controlled::Lease,
}
impl Drop for NativeReceiver {
    fn drop(&mut self) {
        self.observed.borrow_mut().shell.outputs[self.id].drops += 1;
    }
}
/// Strict source query with separately consumed input and expected fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Source operation branch.
    pub group: String,
    /// Branch-specific input, decoded by its consumer.
    pub input: Value,
    /// Independently recorded observation.
    pub expected: Value,
}
/// Execution operands recorded independently by the source oracle.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExecInput {
    /// Literal command.
    pub command: String,
    /// Literal working directory.
    pub cwd: String,
    /// Optional finite numeric spelling preserving negative zero.
    pub timeout: Option<String>,
    /// Optional ordered environment pairs.
    pub env: Option<Vec<(String, Option<String>)>>,
    /// Selected signal state.
    pub signal: String,
    /// Authored nullable exit.
    pub exit_code: Option<f64>,
    /// Complete authored chunk sequence.
    pub chunks: Vec<Vec<u8>>,
}
/// Complete independently recorded execution result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExecExpected {
    /// Literal command observed by the author.
    pub command: String,
    /// Literal directory observed by the author.
    pub cwd: String,
    /// Expected timeout spelling.
    pub timeout: Option<String>,
    /// Expected environment pairs, preserving order and missing values.
    pub env: Option<Vec<(String, Option<String>)>>,
    /// State before output.
    pub initial_signal: Option<bool>,
    /// State after output.
    pub final_signal: Option<bool>,
    /// Successful raw chunks.
    pub chunks: Vec<Vec<u8>>,
    /// Nullable authored exit.
    pub exit_code: Option<f64>,
}
/// Authored call observation, including float bits rather than a display spelling.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Operands {
    /// Complete command string.
    pub command: String,
    /// Complete working directory string.
    pub cwd: String,
    /// Optional float bits rendered for the observation channel.
    pub timeout: Option<String>,
    /// Optional ordered environment.
    pub env: Option<Vec<(String, Option<String>)>>,
    /// Initial live flag state.
    pub initial_signal: Option<bool>,
}
/// Independent operations/replacement presence query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeInput {
    /// Whether to return a capability.
    pub operations: bool,
    /// Missing, null or supplied replacement.
    pub result: String,
}
/// Oracle answer shape.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OutcomeExpected {
    /// Answer existence.
    pub returned: bool,
    /// Capability existence.
    pub operations: bool,
    /// Replacement presence and complete document.
    pub result: Replacement,
    /// Exit of the separately invoked capability.
    pub exit_code: Option<f64>,
}
/// Selected replacement presence.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replacement {
    /// Whether the property exists.
    pub presence: String,
    /// Complete replacement document, when present.
    pub value: Option<Value>,
}
/// No-answer source variant.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyInput {
    /// Undefined or null source answer.
    pub value: String,
}
/// No-answer expectation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyExpected {
    /// Answer existence.
    pub returned: bool,
}
/// Authored failure/catch source branch.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FailureInput {
    /// Error origin and catch policy.
    pub kind: String,
    /// Exact authored diagnostic.
    pub message: String,
}
/// Failure or caught-output result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FailureExpected {
    /// Successful chunks, excluding rejected chunks.
    pub chunks: Vec<Vec<u8>>,
    /// Propagated diagnostic, absent when caught.
    pub error: Option<String>,
    /// Exit when the author catches the error.
    pub exit_code: Option<f64>,
}
