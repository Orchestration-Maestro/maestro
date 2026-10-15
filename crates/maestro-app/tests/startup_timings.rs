//! Startup timing state and exact diagnostic output.
#![cfg(test)]

use maestro_app::configuration::timings::Timings;
use serde::Deserialize;
use std::{cell::Cell, io};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    input: Input,
    expected: Expected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    enabled: Option<String>,
    clock: Vec<f64>,
    actions: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Expected {
    stderr: String,
    clock_reads: usize,
}
#[test]
fn timing_state_preserves_gates_deltas_and_reset() {
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "fixtures/configuration/timing_state_preserves_gates_deltas_and_reset.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 7);
    for case in cases {
        let reads = Cell::new(0);
        let now = || {
            let index = reads.get();
            reads.set(index + 1);
            case.input.clock[index]
        };
        let mut timings = Timings::new(case.input.enabled.as_deref(), now);
        let mut stderr = Vec::new();
        for action in &case.input.actions {
            match action.as_str() {
                "print" => timings.print_timings(&mut stderr).unwrap(),
                "reset" => timings.reset_timings(now),
                time => timings.time(time.strip_prefix("time:").unwrap(), now),
            }
        }
        assert_eq!(String::from_utf8(stderr).unwrap(), case.expected.stderr);
        assert_eq!(reads.get(), case.expected.clock_reads);
    }
}

#[test]
fn timing_writer_propagates_failure() {
    struct FailedWriter;
    impl io::Write for FailedWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::from_raw_os_error(28))
        }
        fn flush(&mut self) -> io::Result<()> {
            panic!("printing does not flush")
        }
    }
    let mut timings = Timings::new(Some("1"), || 10.0);
    timings.time("load", || 12.0);
    assert_eq!(
        timings
            .print_timings(&mut FailedWriter)
            .unwrap_err()
            .raw_os_error(),
        Some(28)
    );
}
