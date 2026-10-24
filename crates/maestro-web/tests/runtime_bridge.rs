//! Exact injected source for both sandbox channels.
use maestro_web::RuntimeMessageBridge;
use maestro_web::components::sandbox::routing::{
    RuntimeMessageBridgeOptions, RuntimeMessageContext,
};
use serde::Deserialize;

/// Complete source-derived input and expected output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BridgeCase {
    /// Execution channel.
    context: String,
    /// Unmodified caller identifier.
    sandbox_id: String,
    /// Complete injectable source.
    expected: String,
}

#[test]
fn bridge_sources_quote_sandbox_identifiers() {
    let cases: Vec<BridgeCase> =
        serde_json::from_str(include_str!("fixtures/runtime_bridges.json")).unwrap();
    assert_eq!(cases.len(), 26);
    for (index, case) in cases.into_iter().enumerate() {
        let context = match case.context.as_str() {
            "sandbox-iframe" => RuntimeMessageContext::SandboxIframe,
            "user-script" => RuntimeMessageContext::UserScript,
            _ => panic!("unknown fixture context at row {index}"),
        };
        let actual = RuntimeMessageBridge::generate_bridge_code(RuntimeMessageBridgeOptions {
            context,
            sandbox_id: &case.sandbox_id,
        })
        .unwrap();
        assert_eq!(actual, case.expected, "source row {index}");
    }
}
