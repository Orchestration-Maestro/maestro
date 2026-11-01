//! Recorded write cases, manual future polling and ordered journals for the write and queue tests.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use maestro_models::{BoxFuture, Cancellation, DiagnosticCode, DiagnosticErrorInfo};
use maestro_tools::{WriteOperations, WriteToolInput};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Waker},
};
use tokio::sync::oneshot;

/// One recorded input and its independently recorded expectation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// Operation-specific input.
    pub input: Value,
    /// Complete expectation.
    pub expected: Value,
}

/// The committed corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Theme keys, whose index plus one is each key's palette color.
    theme_keys: Vec<String>,
    /// Recorded rows by owning test.
    tests: BTreeMap<String, Vec<Row>>,
}

/// Every group the corpus may hold; a group outside this list fails the test that loads it.
const GROUPS: [&str; 21] = [
    "write_titles_preserve_display_alias_precedence",
    "write_content_preview_preserves_display_whitespace",
    "write_preview_limits_count_before_terminal_wrapping",
    "write_preview_uses_live_expand_keybindings",
    "write_highlighting_follows_explicit_selected_path",
    "write_results_render_only_nonempty_errors",
    "write_definition_keeps_schema_and_prompt_metadata",
    "write_input_fields_follow_shared_preflight_validation",
    "write_streaming_cache_rebuilds_or_appends_at_owned_boundaries",
    "write_renderers_reuse_call_and_result_components",
    "write_reports_written_utf8_bytes",
    "write_resolves_paths_before_filesystem_work",
    "mutation_queue_raw_names_preserve_literal_path_identity",
    "mutation_queue_serializes_existing_symlink_aliases",
    "mutation_queue_keeps_invocation_order_before_waiting",
    "mutation_queue_different_paths_can_finish_out_of_order",
    "mutation_queue_failure_does_not_poison_following_work",
    "write_abort_releases_queue_and_drops_pending_operation",
    "write_propagates_adapter_failures_without_progress",
    "write_preaborted_call_starts_no_operations",
    "write_abort_during_mkdir_skips_write_after_queue_release",
];

/// Parse the corpus, rejecting groups outside [`GROUPS`].
fn corpus() -> Corpus {
    let corpus: Corpus =
        serde_json::from_str(include_str!("../fixtures/write-cases.json")).unwrap();
    for group in corpus.tests.keys() {
        assert!(GROUPS.contains(&group.as_str()), "unknown group {group}");
    }
    corpus
}

/// The theme keys of the corpus.
pub fn theme_keys() -> Vec<String> {
    corpus().theme_keys
}

/// The nonempty group of one test.
pub fn rows(test: &str) -> Vec<Row> {
    let rows = corpus().tests.remove(test).unwrap_or_default();
    assert!(
        GROUPS.contains(&test) && !rows.is_empty(),
        "empty group {test}"
    );
    rows
}

/// The expectation of a group holding exactly one row whose input is `{}`.
pub fn single_expected(test: &str) -> Value {
    let mut rows = rows(test);
    assert_eq!(rows.len(), 1, "group {test} must hold one row");
    let row = rows.remove(0);
    assert_eq!(
        row.input,
        serde_json::json!({}),
        "group {test} takes no input"
    );
    row.expected
}

/// Poll `future` once with a waker that does nothing, so progress happens only when the test says.
pub fn poll_once<T>(future: &mut BoxFuture<T>) -> Poll<T> {
    Pin::new(future).poll(&mut Context::from_waker(Waker::noop()))
}

/// Poll `future` once and return its output, failing if it is still pending.
pub fn ready<T>(future: &mut BoxFuture<T>) -> T {
    match poll_once(future) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("future is still pending"),
    }
}

/// An ordered record of events shared with spawned operations.
#[derive(Clone, Default)]
pub struct Journal(Arc<Mutex<Vec<String>>>);

impl Journal {
    /// Append one event.
    pub fn push(&self, event: impl Into<String>) {
        self.0.lock().unwrap().push(event.into());
    }

    /// The events so far.
    pub fn entries(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

/// The sending half of a gate; operations wait on the receiving half until [`Gate::open`].
pub struct Gate(oneshot::Sender<()>);

impl Gate {
    /// A closed gate and the future its operation awaits.
    pub fn new() -> (Self, oneshot::Receiver<()>) {
        let (sender, receiver) = oneshot::channel();
        (Self(sender), receiver)
    }

    /// Let the waiting operation continue the next time it is polled.
    pub fn open(self) {
        self.0.send(()).unwrap();
    }
}

/// What a controlled operation does when called.
#[derive(Clone)]
pub enum Mode {
    /// Complete normally.
    Succeed,
    /// Fail with this diagnostic.
    Fail(DiagnosticErrorInfo),
    /// Abort the write's signal, then complete normally.
    AbortThenSucceed,
    /// Abort the write's signal, then fail.
    AbortThenFail(DiagnosticErrorInfo),
    /// Never complete, counting drops of the returned future.
    Pend,
}

/// A future that never completes and counts being dropped.
struct PendProbe(Arc<AtomicUsize>);

impl Future for PendProbe {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
        Poll::Pending
    }
}

impl Drop for PendProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// The diagnostic a failing controlled operation reports.
pub fn failure(message: &str) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: Some("ControlledFailure".into()),
        message: message.into(),
        stack: Some("controlled stack".into()),
        code: Some(DiagnosticCode::Text("E_CONTROL".into())),
    }
}

/// Controlled write operations that record every call.
pub struct ScriptedWrite {
    /// Events of the operations: `mkdir` or `write` on a call, `mkdir:start` or `write:start` when it pends.
    pub journal: Journal,
    /// Behavior of directory creation.
    mkdir: Mode,
    /// Behavior of the file write.
    write: Mode,
    /// Signal aborted by the abort modes.
    signal: Option<Cancellation>,
    /// Calls as `[operation, operands...]`.
    calls: Mutex<Vec<Vec<String>>>,
    /// Drops of pending futures.
    pub dropped: Arc<AtomicUsize>,
}

impl ScriptedWrite {
    /// Operations that always succeed.
    pub fn new(journal: &Journal) -> Self {
        Self {
            journal: journal.clone(),
            mkdir: Mode::Succeed,
            write: Mode::Succeed,
            signal: None,
            calls: Mutex::new(Vec::new()),
            dropped: Arc::default(),
        }
    }

    /// Behave as `mode` when creating directories.
    pub fn mkdir_mode(mut self, mode: Mode) -> Self {
        self.mkdir = mode;
        self
    }

    /// Behave as `mode` when writing the file.
    pub fn write_mode(mut self, mode: Mode) -> Self {
        self.write = mode;
        self
    }

    /// Abort `signal` in the abort modes.
    pub fn signal(mut self, signal: &Cancellation) -> Self {
        self.signal = Some(signal.clone());
        self
    }

    /// The calls made, in order.
    pub fn calls(&self) -> Vec<Vec<String>> {
        self.calls.lock().unwrap().clone()
    }

    /// The operation names called, in order.
    pub fn names(&self) -> Vec<String> {
        self.calls()
            .into_iter()
            .map(|call| call[0].clone())
            .collect()
    }

    /// Record the call and apply `mode`.
    pub fn step(
        &self,
        name: &str,
        operands: &[&str],
        mode: &Mode,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        let mut call = vec![name.to_owned()];
        call.extend(operands.iter().map(|operand| (*operand).to_owned()));
        self.calls.lock().unwrap().push(call);
        let pending = matches!(mode, Mode::Pend);
        self.journal.push(if pending {
            format!("{name}:start")
        } else {
            name.to_owned()
        });
        if matches!(mode, Mode::AbortThenSucceed | Mode::AbortThenFail(_)) {
            self.signal.as_ref().unwrap().abort();
        }
        match mode.clone() {
            Mode::Succeed | Mode::AbortThenSucceed => Box::pin(std::future::ready(Ok(()))),
            Mode::Fail(error) | Mode::AbortThenFail(error) => {
                Box::pin(std::future::ready(Err(error)))
            }
            Mode::Pend => {
                let probe = PendProbe(Arc::clone(&self.dropped));
                Box::pin(async move {
                    probe.await;
                    Ok(())
                })
            }
        }
    }
}

impl WriteOperations for ScriptedWrite {
    fn write_file(
        &self,
        absolute_path: String,
        content: String,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        self.step("write", &[&absolute_path, &content], &self.write)
    }

    fn mkdir(&self, dir: String) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        self.step("mkdir", &[&dir], &self.mkdir)
    }
}

/// Write arguments with the given operands.
pub fn input(path: &str, content: &str) -> WriteToolInput {
    WriteToolInput {
        path: path.into(),
        content: content.into(),
    }
}

/// The lowercase hexadecimal spelling of `bytes`.
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        write!(out, "{byte:02x}").unwrap();
        out
    })
}
