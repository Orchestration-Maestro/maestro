//! Untrusted conversation JSON is rejected before native provider admission.
use crate::{
    local_resources::{Pump, model_streams::Creation},
    model_calls::Controls,
};
use serde_json::json;

#[test]
fn model_start_rejects_unknown_roles_and_content_before_dispatch() {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let registry = pump.registry();
    let model = json!({
        "id":"claude", "name":"Claude", "api":"bedrock-converse-stream", "provider":"amazon-bedrock",
        "baseUrl":"https://example.invalid", "reasoning":false, "input":["text"],
        "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0}, "contextWindow":1000,"maxTokens":100,
    });
    let mut assistant = serde_json::to_value(maestro_models::faux_assistant_message(
        "",
        maestro_models::FauxAssistantMessageOptions::default(),
    ))
    .unwrap();
    assistant["content"] = json!([{"type":"future-block"}]);
    for (message, diagnostic) in [
        (assistant, "unknown variant `future-block`"),
        (
            json!({"role":"future-role", "content":"ignored", "timestamp":0}),
            "unknown variant `future-role`",
        ),
        (
            json!({"role":"user", "content":[{"type":"future-block"}], "timestamp":0}),
            "did not match any variant",
        ),
        (
            json!({"role":"user", "content":[{"type":"text","text":"kept"},{"type":"future-block"}], "timestamp":0}),
            "did not match any variant",
        ),
    ] {
        for simple in [false, true] {
            let data = json!([model, {"messages":[message]}, null]).to_string();
            let (reply, response) = std::sync::mpsc::channel();
            Creation {
                simple,
                data,
                controls: Controls {
                    signal: None,
                    payload: None,
                    response: None,
                    objects: None,
                },
                reply,
            }
            .answer(&registry);
            let Err(error) = response.recv().unwrap() else {
                panic!("invalid conversation admitted")
            };
            assert!(error.contains(diagnostic), "{error}");
        }
    }
    drop((registry, admission, pump));
}
