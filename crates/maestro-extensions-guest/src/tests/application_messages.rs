//! Application messages through both author adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::documents::{Answer, ask};
use super::scenario::Driver;
use serde_json::json;

/// Carries application roles without executing or filtering their contents.
async fn roles(driver: &mut impl Driver) -> Result<(), String> {
    let messages = [
        json!({"role":"bashExecution","command":"echo Ω","output":"literal","cancelled":false,"truncated":true,"timestamp":4.5}),
        json!({"role":"custom","customType":"notice","content":"opaque","display":false,"timestamp":5.5}),
        json!({"role":"branchSummary","summary":"branch","fromId":"ancestor","timestamp":6.5}),
        json!({"role":"compactionSummary","summary":"compact","tokensBefore":-0.0,"timestamp":7.5}),
    ];
    for message in messages {
        let event = json!({"type":"message_end","message":message});
        assert_eq!(
            ask(driver, &event, &json!({})).await?,
            Answer::returned(&event, None)
        );
    }
    optional_roles(driver).await
}

/// Optional application fields retain missing and explicit null independently.
async fn optional_roles(driver: &mut impl Driver) -> Result<(), String> {
    for field in ["exitCode", "fullOutputPath", "excludeFromContext"] {
        for value in [serde_json::Value::Null, json!("populated")] {
            let value = if value.is_null() {
                value
            } else {
                match field {
                    "exitCode" => json!(-1.5),
                    "excludeFromContext" => json!(false),
                    _ => json!(" /../full Ω "),
                }
            };
            let mut message = json!({"role":"bashExecution","command":"","output":"","cancelled":true,"truncated":false,"timestamp":1.0});
            message[field] = value;
            let event = json!({"type":"message_start","message":message});
            assert_eq!(
                ask(driver, &event, &json!({})).await?,
                Answer::returned(&event, None)
            );
        }
    }
    for content in [
        json!([]),
        json!([{"type":"text","text":"z"},{"type":"image","data":"AA==","mimeType":"image/png"},{"type":"text","text":"z"}]),
    ] {
        for details in [
            None,
            Some(serde_json::Value::Null),
            Some(json!(" {\"x\":1,\"x\":2} ")),
        ] {
            let mut message = json!({"role":"custom","customType":"","content":content,"display":true,"timestamp":2.0});
            if let Some(details) = details {
                message["details"] = details;
            }
            let event = json!({"type":"agent_end","messages":[message.clone(),message]});
            assert_eq!(
                ask(driver, &event, &json!({})).await?,
                Answer::returned(&event, None)
            );
        }
    }
    Ok(())
}

#[test]
fn maestro_message_roles_keep_content_and_optional_fields() -> Result<(), String> {
    on_both_adapters!(roles)
}
