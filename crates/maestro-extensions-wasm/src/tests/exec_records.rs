//! Explicit-execution records at the typed JSON boundary.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::types::finite::encode_value;
use crate::{ExecOptions, ExecResult};

#[test]
fn exec_records_keep_optional_fields_and_finite_numbers() {
    let result: ExecResult =
        serde_json::from_str(r#"{"stdout":"out\u0000\r\n","stderr":"","code":-7,"killed":true}"#)
            .unwrap();
    assert_eq!(
        result,
        ExecResult {
            stdout: "out\0\r\n".into(),
            stderr: String::new(),
            code: -7,
            killed: true,
        }
    );
    assert!(serde_json::from_str::<ExecResult>(r#"{"stdout":"","stderr":"","code":0}"#).is_err());

    assert_eq!(encode_value(&ExecOptions::default()).unwrap(), "{}");
    let cwd = ExecOptions {
        cwd: Some(String::new()),
        ..ExecOptions::default()
    };
    assert_eq!(encode_value(&cwd).unwrap(), r#"{"cwd":""}"#);
    for timeout in [0.0_f64, -0.0, -1.0, 0.5, 2500.0] {
        let options = ExecOptions {
            timeout: Some(timeout),
            ..ExecOptions::default()
        };
        let encoded: serde_json::Value =
            serde_json::from_str(&encode_value(&options).unwrap()).unwrap();
        let decoded = encoded["timeout"].as_f64().unwrap();
        assert_eq!(decoded.to_bits(), timeout.to_bits());
    }
    for timeout in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        let options = ExecOptions {
            timeout: Some(timeout),
            ..ExecOptions::default()
        };
        assert_eq!(
            encode_value(&options),
            Err("extension wrote a non-finite number (Infinity or NaN)".to_owned())
        );
    }
}
