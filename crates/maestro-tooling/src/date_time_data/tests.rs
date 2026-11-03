//! Malformed supplemental data must fail before assets can be published.
use super::*;

/// Unknown hour skeletons identify the offending locale and value.
#[test]
fn date_time_data_rejects_unknown_preferred_hours() {
    let directory =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../maestro-text/data/date-time/inputs");
    let mut inputs = inputs(&directory).unwrap();
    inputs.insert(
        "timeData.json".into(),
        br#"{"supplemental":{"timeData":{"en-US":{"_preferred":"unknown"}}}}"#.to_vec(),
    );

    let error = assets(&inputs)
        .expect_err("unknown preferred hours must be refused")
        .to_string();

    assert!(error.contains("en-US"), "{error}");
    assert!(error.contains("unknown"), "{error}");
}
