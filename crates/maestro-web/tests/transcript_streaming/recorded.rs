//! Recorded deferred-publication operands observed through actual assistant rendering.
use super::{AgentMessage, AssistantContent, message, rendered};
use crate::effects_support;
use maestro_web::{StreamingMessageContainer, browser::attachment_content::AttachmentHost};
use serde_json::Value;
use std::rc::Rc;

/// Convert recorded assistant content to the existing highlighter observation surface.
fn source(content: &Value) -> AgentMessage<super::BrowserMessages<super::NoCustomMessages>> {
    let value = message("");
    let AgentMessage::Assistant(assistant) = &value else {
        unreachable!()
    };
    let mut content: Vec<AssistantContent> = serde_json::from_value(content.clone()).unwrap();
    for item in &mut content {
        if let AssistantContent::Text(text) = item {
            text.text = format!("```javascript\n{}\n```", text.text);
        }
    }
    assistant.write().unwrap().content = content;
    value
}
/// Mutate the recorded pending tool argument through its original shared message owner.
fn mutate(value: &AgentMessage<super::BrowserMessages<super::NoCustomMessages>>) {
    let AgentMessage::Assistant(assistant) = value else {
        unreachable!()
    };
    let mut assistant = assistant.write().unwrap();
    let AssistantContent::ToolCall(call) = &mut assistant.content[0] else {
        unreachable!()
    };
    call.arguments["nested"]["value"] = 2.into();
}
/// Every recorded action is checked before and after delivery, including cancellation and next turns.
#[test]
fn recorded_streaming_sequences_publish_expected_content() {
    let corpus: Value = serde_json::from_str(include_str!("../fixtures/transcript.json")).unwrap();
    for row in corpus["streaming"].as_array().unwrap() {
        let clock = Rc::new(effects_support::Host::default());
        let host: Rc<dyn AttachmentHost> = clock.clone();
        let mut container = StreamingMessageContainer::new(host.clone());
        let mut latest = None;
        let mut frame = 0;
        for step in row["steps"].as_array().unwrap() {
            let action = &step["action"];
            if action["frame"] == true {
                frame += 1;
                clock.advance(frame);
            } else if action["mutate"] == true {
                mutate(latest.as_ref().unwrap());
            } else {
                latest = (!action["message"].is_null()).then(|| source(&action["message"]));
                container
                    .set_message(latest.clone(), action["immediate"].as_bool().unwrap())
                    .unwrap();
            }
            let expected: Vec<String> = serde_json::from_value(step["expected"].clone()).unwrap();
            assert_eq!(
                rendered(&mut container, &host),
                expected,
                "{}: {action}",
                row["id"]
            );
        }
    }
}
