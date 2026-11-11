//! Channel selection and identifier quoting of the injectable runtime script.
use maestro_web::RuntimeMessageBridge;
use maestro_web::components::sandbox::routing::{
    RuntimeMessageBridgeOptions, RuntimeMessageContext,
};
use maestro_web::components::sandbox::runtime_bundle::BUNDLE;
use serde::Deserialize;

/// Complete source-derived input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BridgeCase {
    /// Execution channel.
    context: String,
    /// Unmodified caller identifier.
    sandbox_id: String,
}

#[test]
fn bridge_sources_quote_sandbox_identifiers() {
    let cases: Vec<BridgeCase> =
        serde_json::from_str(include_str!("fixtures/runtime_bridges.json")).unwrap();
    assert_eq!(cases.len(), 26);
    for (index, case) in cases.into_iter().enumerate() {
        let (context, role) = match case.context.as_str() {
            "sandbox-iframe" => (RuntimeMessageContext::SandboxIframe, "frame-bridge"),
            "user-script" => (RuntimeMessageContext::UserScript, "user-script"),
            _ => panic!("unknown fixture context at row {index}"),
        };
        let actual = RuntimeMessageBridge::generate_bridge_code(RuntimeMessageBridgeOptions {
            context,
            sandbox_id: &case.sandbox_id,
        })
        .unwrap();
        let rest = actual
            .strip_prefix("var Module={arguments:")
            .unwrap_or_else(|| panic!("arguments prefix at row {index}"));
        let mut values = serde_json::Deserializer::from_str(rest).into_iter::<serde_json::Value>();
        let arguments = values.next().unwrap().unwrap();
        assert_eq!(
            arguments,
            serde_json::json!([role, case.sandbox_id]),
            "arguments row {index}"
        );
        let remainder = &rest[values.byte_offset()..];
        assert_eq!(remainder, format!("}};{BUNDLE}"), "script row {index}");
    }
}
