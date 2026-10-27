//! Delivery records decode the same optional fields that authoring encodes.
use maestro_request::messages::{SendMessageOptions, SendUserMessageOptions};
use serde_json::{Value, json};

#[test]
fn custom_delivery_options_preserve_empty_and_explicit_values() {
    for expected in [
        json!({}),
        json!({"triggerTurn": false}),
        json!({"triggerTurn": true, "deliverAs": "nextTurn"}),
        json!({"deliverAs": "steer"}),
        json!({"deliverAs": "followUp"}),
    ] {
        let decoded: SendMessageOptions = serde_json::from_value(expected.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), expected);
    }
    let absent: Option<SendMessageOptions> = serde_json::from_value(Value::Null).unwrap();
    assert!(absent.is_none());
}

#[test]
fn user_delivery_options_preserve_empty_and_supported_placements() {
    for expected in [
        json!({}),
        json!({"deliverAs": "steer"}),
        json!({"deliverAs": "followUp"}),
    ] {
        let decoded: SendUserMessageOptions = serde_json::from_value(expected.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), expected);
    }
    assert!(
        serde_json::from_value::<SendUserMessageOptions>(json!({"deliverAs": "nextTurn"})).is_err()
    );
    assert!(serde_json::from_value::<SendMessageOptions>(json!({"triggerTurn": "true"})).is_err());
}
