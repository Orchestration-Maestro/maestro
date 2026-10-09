//! Response-session behavior witnesses.

mod requests;

mod streams;

/// Build the controlled request descriptor without consulting a catalog or environment.
fn invocation() -> (
    std::sync::Arc<crate::Model>,
    crate::Context,
    super::OpenAICodexResponsesOptions,
) {
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("tests/fixtures/defaults.json")).unwrap();
    let model = serde_json::from_value(rows[0]["model"].clone()).unwrap();
    let context = serde_json::from_value(rows[0]["context"].clone()).unwrap();
    let options = super::OpenAICodexResponsesOptions {
        common: crate::StreamOptions {
            api_key: Some("a.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjX3Rlc3QifX0=.b".to_owned()),
            ..Default::default()
        }, ..Default::default()
    };
    (std::sync::Arc::new(model), context, options)
}
