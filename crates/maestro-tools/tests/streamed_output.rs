//! Native streamed-output observations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use maestro_tools::TruncationBy;
use maestro_tools::output::output_accumulator::{OutputAccumulator, OutputAccumulatorOptions};
use serde::Deserialize;
use serde_json::{Value, json};

/// A native budget or the corpus's unbounded marker.
#[derive(Deserialize)]
#[serde(untagged)]
enum Budget {
    /// Native integer budget.
    Count(usize),
    /// Explicit unlimited marker.
    Marker(String),
}
impl Budget {
    /// Resolve the marker without silently accepting unknown text.
    fn resolve(self) -> usize {
        match self {
            Self::Count(value) => value,
            Self::Marker(value) => {
                assert_eq!(value, "unlimited");
                usize::MAX
            }
        }
    }
}
/// Corpus constructor controls.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Options {
    /// Independent line budget.
    max_lines: Option<Budget>,
    /// Independent byte budget.
    max_bytes: Option<Budget>,
    /// Authored filename prefix.
    temp_file_prefix: Option<String>,
}
/// Recorded invocation.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Case {
    /// Test owning this invocation.
    test: String,
    /// Input class for failure attribution.
    rule: String,
    /// Constructor input.
    options: Options,
    /// Raw chunks encoded in hex.
    chunks: Vec<String>,
    /// Number of decoder finishes.
    finish_count: Option<usize>,
    /// Whether each observation includes persistence probes.
    #[serde(default)]
    repeat_snapshots: bool,
    /// Invalid append after finish, including empty input.
    append_after_finish: Option<String>,
    /// Independently recorded observations.
    expected: Expected,
}
/// Complete expected operation result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// All ordered snapshots and current line counts.
    observations: Vec<Value>,
    /// Original file bytes after close.
    artifact: Option<String>,
    /// Authored append error.
    error: Option<String>,
}
/// Decode corpus raw bytes.
fn bytes(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0);
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}
/// Capture one snapshot, retaining path identity separately from nondeterminism.
fn observe(a: &mut OutputAccumulator, persist: bool, path: &mut Option<String>) -> Value {
    let mut snapshot = serde_json::to_value(a.snapshot(persist).unwrap()).unwrap();
    for field in ["maxLines", "maxBytes"] {
        let budget = &mut snapshot["truncation"][field];
        if *budget == json!(usize::MAX) {
            *budget = json!("unlimited");
        }
    }
    if let Some(actual) = snapshot.get_mut("fullOutputPath") {
        let actual_path = actual.as_str().unwrap();
        if let Some(previous) = path.as_ref() {
            assert_eq!(actual_path, previous);
        } else {
            *path = Some(actual_path.to_owned());
        }
        *actual = json!("$ARTIFACT");
    }
    json!({"snapshot": snapshot, "lastLineBytes": a.get_last_line_bytes()})
}
/// Observe the same operation with each requested persistence mode.
fn observations(
    a: &mut OutputAccumulator,
    repeat: bool,
    path: &mut Option<String>,
    out: &mut Vec<Value>,
) {
    out.push(observe(a, false, path));
    if repeat {
        for persist in [true, false] {
            out.push(observe(a, persist, path));
        }
    }
}
/// Assert artifact name, path identity and original bytes after completed writes.
fn artifact(path: Option<String>, expected: Option<String>, prefix: &str) {
    assert_eq!(path.is_some(), expected.is_some());
    if let (Some(path), Some(expected)) = (path, expected) {
        let filename = path.strip_suffix(".log").unwrap();
        let (_, id) = filename.rsplit_once('-').unwrap();
        assert_eq!(id.len(), 16);
        assert!(
            id.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        let root = std::env::temp_dir();
        assert_eq!(
            path,
            maestro_path::join(&[root.to_str().unwrap(), &format!("{prefix}-{id}.log")])
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes(&expected));
        std::fs::remove_file(path).unwrap();
    }
}
/// Registered corpus groups, each owned by one test function.
const CORPUS_GROUPS: &[&str] = &[
    "empty_stream_keeps_initial_counts",
    "constructor_limits_apply_independently",
    "utf8_scalars_survive_every_chunk_split",
    "leading_bom_is_removed_once_across_chunks",
    "invalid_utf8_uses_maximal_subpart_replacement",
    "finish_flushes_incomplete_sequence_once",
    "finish_can_trigger_decoded_only_spill",
    "decoded_overflow_spills_original_bytes",
    "raw_overflow_can_leave_display_untruncated",
    "newline_and_last_line_counts_use_decoded_bytes",
    "byte_spill_uses_strict_excess",
    "line_only_overflow_spills_all_chunks",
    "zero_limits_keep_tail_accounting",
    "unlimited_limits_do_not_overflow_arithmetic",
    "spill_replays_raw_prefix_once",
    "rolling_tail_preserves_hysteresis",
    "trimmed_leading_fragment_is_not_a_complete_line",
    "rolling_trim_aligns_utf8_boundaries",
    "snapshot_reconciles_global_counts_and_reason",
    "partial_last_line_keeps_byte_reason",
    "snapshots_do_not_finish_or_duplicate_output",
    "append_after_finish_preserves_state",
    "artifact_names_keep_prefix_and_suffix",
];
/// Execute each selected row exactly once, consuming every operation control.
fn run(name: &str) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/streamed_output.json")).unwrap();
    assert_eq!(cases.len(), 151);
    assert!(
        cases
            .iter()
            .all(|case| CORPUS_GROUPS.contains(&case.test.as_str()))
    );
    let mut selected = 0;
    for case in cases.into_iter().filter(|case| case.test == name) {
        selected += 1;
        let prefix = case
            .options
            .temp_file_prefix
            .as_deref()
            .unwrap_or("maestro-output")
            .to_owned();
        let mut a = OutputAccumulator::new(OutputAccumulatorOptions {
            max_lines: case.options.max_lines.map(Budget::resolve),
            max_bytes: case.options.max_bytes.map(Budget::resolve),
            temp_file_prefix: case.options.temp_file_prefix,
        });
        let mut path = None;
        let mut actual = Vec::new();
        observations(&mut a, case.repeat_snapshots, &mut path, &mut actual);
        for chunk in case.chunks {
            a.append(&bytes(&chunk)).unwrap();
            observations(&mut a, case.repeat_snapshots, &mut path, &mut actual);
        }
        for _ in 0..case.finish_count.unwrap_or(1) {
            a.finish().unwrap();
            observations(&mut a, case.repeat_snapshots, &mut path, &mut actual);
        }
        let error = case.append_after_finish.map(|chunk| {
            let error = a.append(&bytes(&chunk)).unwrap_err().to_string();
            observations(&mut a, case.repeat_snapshots, &mut path, &mut actual);
            error
        });
        assert_eq!(error, case.expected.error, "{}", case.rule);
        assert_eq!(actual, case.expected.observations, "{}", case.rule);
        let close: fn(&mut OutputAccumulator) -> std::io::Result<()> =
            OutputAccumulator::close_temp_file;
        close(&mut a).unwrap();
        artifact(path, case.expected.artifact, &prefix);
    }
    assert!(selected > 0, "unknown test group");
}

#[test]
fn empty_stream_keeps_initial_counts() {
    run("empty_stream_keeps_initial_counts");
}
#[test]
fn constructor_limits_apply_independently() {
    run("constructor_limits_apply_independently");
}
#[test]
fn utf8_scalars_survive_every_chunk_split() {
    run("utf8_scalars_survive_every_chunk_split");
}
#[test]
fn leading_bom_is_removed_once_across_chunks() {
    run("leading_bom_is_removed_once_across_chunks");
}
#[test]
fn invalid_utf8_uses_maximal_subpart_replacement() {
    run("invalid_utf8_uses_maximal_subpart_replacement");
    let mut output = OutputAccumulator::new(OutputAccumulatorOptions {
        max_bytes: Some(1),
        ..Default::default()
    });
    output.append(&[0xff, 0xff, 0xe2, 0x82, 0xac]).unwrap();
    output.finish().unwrap();
    let snapshot = output.snapshot(false).unwrap();
    assert_eq!(snapshot.content, "");
    assert_eq!(snapshot.truncation.total_bytes, 9);
    assert!(!snapshot.truncation.last_line_partial);
    let path = snapshot.full_output_path.unwrap();
    output.close_temp_file().unwrap();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        [0xff, 0xff, 0xe2, 0x82, 0xac]
    );
    std::fs::remove_file(path).unwrap();
}
#[test]
fn finish_flushes_incomplete_sequence_once() {
    run("finish_flushes_incomplete_sequence_once");
}

#[test]
fn finish_can_trigger_decoded_only_spill() {
    run("finish_can_trigger_decoded_only_spill");
}

#[test]
fn decoded_overflow_spills_original_bytes() {
    run("decoded_overflow_spills_original_bytes");
}

#[test]
fn raw_overflow_can_leave_display_untruncated() {
    run("raw_overflow_can_leave_display_untruncated");
}

#[test]
fn newline_and_last_line_counts_use_decoded_bytes() {
    run("newline_and_last_line_counts_use_decoded_bytes");
}

#[test]
fn byte_spill_uses_strict_excess() {
    run("byte_spill_uses_strict_excess");
}

#[test]
fn line_only_overflow_spills_all_chunks() {
    run("line_only_overflow_spills_all_chunks");
}

#[test]
fn zero_limits_keep_tail_accounting() {
    run("zero_limits_keep_tail_accounting");
}
#[test]
fn rolling_tail_preserves_hysteresis() {
    run("rolling_tail_preserves_hysteresis");
}

#[test]
fn unlimited_limits_do_not_overflow_arithmetic() {
    run("unlimited_limits_do_not_overflow_arithmetic");
}

#[test]
fn spill_replays_raw_prefix_once() {
    run("spill_replays_raw_prefix_once");
}

#[test]
fn trimmed_leading_fragment_is_not_a_complete_line() {
    run("trimmed_leading_fragment_is_not_a_complete_line");
}

#[test]
fn rolling_trim_aligns_utf8_boundaries() {
    run("rolling_trim_aligns_utf8_boundaries");
}

#[test]
fn snapshot_reconciles_global_counts_and_reason() {
    run("snapshot_reconciles_global_counts_and_reason");
}

#[test]
fn partial_last_line_keeps_byte_reason() {
    run("partial_last_line_keeps_byte_reason");
}

#[test]
fn snapshots_do_not_finish_or_duplicate_output() {
    run("snapshots_do_not_finish_or_duplicate_output");
}

#[test]
fn append_after_finish_preserves_state() {
    run("append_after_finish_preserves_state");
}
#[test]
fn artifact_names_keep_prefix_and_suffix() {
    run("artifact_names_keep_prefix_and_suffix");
}

/// Prefix below a parent that cannot exist beneath a regular file.
fn missing_parent() -> (std::path::PathBuf, String) {
    let path = std::env::temp_dir().join(format!(
        "maestro-parent-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    drop(file);
    let prefix = path.file_name().unwrap().to_str().unwrap();
    let prefix = format!("{prefix}/output");
    (path, prefix)
}

#[test]
fn small_output_never_opens_artifact() {
    let (parent, prefix) = missing_parent();
    let mut a = OutputAccumulator::new(OutputAccumulatorOptions {
        max_bytes: Some(2),
        temp_file_prefix: Some(prefix),
        ..Default::default()
    });
    assert!(a.snapshot(true).unwrap().full_output_path.is_none());
    a.append(&[0xe2, 0x82]).unwrap();
    assert!(a.snapshot(true).unwrap().full_output_path.is_none());
    a.close_temp_file().unwrap();
    std::fs::remove_file(parent).unwrap();
}

#[test]
fn artifact_creation_failure_is_returned() {
    let (parent, prefix) = missing_parent();
    let options = || OutputAccumulatorOptions {
        max_bytes: Some(2),
        temp_file_prefix: Some(prefix.clone()),
        ..Default::default()
    };
    let mut append = OutputAccumulator::new(options());
    assert!(append.append(b"abc").is_err());
    let mut finish = OutputAccumulator::new(options());
    finish.append(&[0xe2, 0x82]).unwrap();
    assert!(finish.finish().is_err());
    let mut snapshot = OutputAccumulator::new(OutputAccumulatorOptions {
        max_lines: Some(0),
        ..options()
    });
    assert!(snapshot.snapshot(true).is_ok());
    assert!(snapshot.append(b"x").is_err());
    assert!(snapshot.snapshot(true).is_err());
    std::fs::remove_file(parent).unwrap();
}

#[test]
fn closed_artifact_preserves_output_observations() {
    let mut output = OutputAccumulator::new(OutputAccumulatorOptions {
        max_bytes: Some(4),
        ..Default::default()
    });
    output.append(b"first\nlast").unwrap();
    output.finish().unwrap();
    let before = serde_json::to_value(output.snapshot(true).unwrap()).unwrap();
    let path = before["fullOutputPath"].as_str().unwrap();
    output.close_temp_file().unwrap();
    assert_eq!(output.get_last_line_bytes(), 4);
    for persist in [false, true] {
        assert_eq!(
            serde_json::to_value(output.snapshot(persist).unwrap()).unwrap(),
            before
        );
    }
    output.close_temp_file().unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"first\nlast");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn append_after_artifact_close_preserves_state_and_bytes() {
    let mut output = OutputAccumulator::new(OutputAccumulatorOptions {
        max_bytes: Some(2),
        ..Default::default()
    });
    output.append(b"abc").unwrap();
    let before = serde_json::to_value(output.snapshot(false).unwrap()).unwrap();
    let path = before["fullOutputPath"].as_str().unwrap();
    output.close_temp_file().unwrap();
    for chunk in [b"def".as_slice(), b""] {
        assert!(output.append(chunk).is_err());
        assert_eq!(output.get_last_line_bytes(), 3);
        assert_eq!(
            serde_json::to_value(output.snapshot(true).unwrap()).unwrap(),
            before
        );
    }
    assert_eq!(std::fs::read(path).unwrap(), b"abc");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn rolling_trim_keeps_the_tail_of_an_oversized_line_ending_in_a_newline() {
    let mut output = OutputAccumulator::new(OutputAccumulatorOptions {
        max_bytes: Some(16),
        ..Default::default()
    });
    for _ in 0..100 {
        output.append(&[b'x'; 1000]).unwrap();
    }
    output.append(b"\n").unwrap();
    let snapshot = output.snapshot(false).unwrap();
    assert_eq!(snapshot.content, "x".repeat(16));
    let truncation = &snapshot.truncation;
    assert_eq!(
        (truncation.total_bytes, truncation.total_lines),
        (100_001, 1)
    );
    assert_eq!((truncation.output_bytes, truncation.output_lines), (16, 1));
    assert!(truncation.truncated && truncation.last_line_partial);
    assert!(matches!(truncation.truncated_by, Some(TruncationBy::Bytes)));
    output.close_temp_file().unwrap();
    std::fs::remove_file(snapshot.full_output_path.unwrap()).unwrap();
}
