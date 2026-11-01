//! Application message roles retain their authored JSON through native event boundaries.
use maestro_session::messages::CustomAgentMessages;
use serde_json::json;

#[test]
fn application_roles_roundtrip_without_an_enum_wrapper() {
    for value in [
        json!({"role":"bashExecution","command":"raw","output":"雪","cancelled":false,"truncated":false,"timestamp":1.5,"exitCode":null}),
        json!({"role":"custom","customType":"x","content":"text","display":true,"timestamp":2.0,"details":null}),
        json!({"role":"branchSummary","summary":"branch","fromId":"id","timestamp":3.0}),
        json!({"role":"compactionSummary","summary":"compact","tokensBefore":4.0,"timestamp":5.0}),
    ] {
        let message: CustomAgentMessages = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(message).unwrap(), value);
    }
    assert!(serde_json::from_value::<CustomAgentMessages>(json!({"role":"unknown"})).is_err());
}
