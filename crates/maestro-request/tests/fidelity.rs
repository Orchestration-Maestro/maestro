//! Plain JSON fidelity of the single-owned host records.
use maestro_request::diagnostics::{AssistantMessageDiagnostic, DiagnosticCode};
use maestro_request::types::{
    AssistantMessage, AssistantMessageEvent, Message, Model, ModelCost, RoutingPrice, RoutingSort,
    RoutingThreshold, StopReason, TextContent, Usage, UsageCost, UserContent, UserMessage,
};
use serde_json::{from_str, to_string};

/// Samples every finite exponent with both signs and three significands.
fn sample() -> impl Iterator<Item = f64> {
    // Every finite exponent, both signs, and three significands.
    (0_u64..2047).flat_map(|exponent| {
        [0, 1_u64 << 63].into_iter().flat_map(move |sign| {
            [0, 1, (1_u64 << 52) - 1]
                .into_iter()
                .map(move |mantissa| f64::from_bits(sign | exponent << 52 | mantissa))
        })
    })
}

/// Builds a supplied assistant message with the given timestamp.
fn assistant(timestamp: f64) -> AssistantMessage {
    AssistantMessage {
        content: vec![],
        api: "custom".into(),
        provider: "p".into(),
        model: "m".into(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 0.0,
            cost: UsageCost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
                total: 0.0,
            },
        },
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp,
    }
}

#[test]
/// Preserves finite bits through direct records and message role selection.
fn finite_numbers_keep_bits_through_direct_and_tagged_messages()
-> Result<(), Box<dyn std::error::Error>> {
    let mut count = 0;
    for number in sample() {
        let user = UserMessage {
            content: UserContent::Text("text".into()),
            timestamp: number,
        };
        let received: UserMessage = from_str(&to_string(&user)?)?;
        assert_eq!(received.timestamp.to_bits(), number.to_bits());
        let tagged: Message = from_str(&to_string(&Message::User(user))?)?;
        let Message::User(received) = tagged else {
            panic!("role changed")
        };
        assert_eq!(received.timestamp.to_bits(), number.to_bits());
        count += 1;
    }
    println!("{count} finite bit patterns: direct + tagged messages, exact bits");
    Ok(())
}

#[test]
/// Retains numeric and textual alternatives without coercion.
fn numeric_union_branches_remain_numbers() -> Result<(), Box<dyn std::error::Error>> {
    for number in sample() {
        let value: DiagnosticCode = from_str(&to_string(&DiagnosticCode::Number(number))?)?;
        let DiagnosticCode::Number(value) = value else {
            panic!("number became text")
        };
        assert_eq!(value.to_bits(), number.to_bits());
        let value: RoutingPrice = from_str(&to_string(&RoutingPrice::Number(number))?)?;
        let RoutingPrice::Number(value) = value else {
            panic!("number became text")
        };
        assert_eq!(value.to_bits(), number.to_bits());
        let value: RoutingThreshold = from_str(&to_string(&RoutingThreshold::Number(number))?)?;
        let RoutingThreshold::Number(value) = value else {
            panic!("number became percentiles")
        };
        assert_eq!(value.to_bits(), number.to_bits());
    }
    for text in [
        "1",
        "1.0",
        "-0",
        "-0.0",
        "Infinity",
        "NaN",
        "f64:7ff0000000000000",
    ] {
        let value: DiagnosticCode = from_str(&to_string(&DiagnosticCode::Text(text.into()))?)?;
        assert!(matches!(value, DiagnosticCode::Text(value) if value == text));
        let value: RoutingPrice = from_str(&to_string(&RoutingPrice::Text(text.into()))?)?;
        assert!(matches!(value, RoutingPrice::Text(value) if value == text));
    }
    Ok(())
}

#[test]
/// Preserves finite bits through model selection, stream handles and diagnostics.
fn finite_numbers_survive_model_buffering_and_stream_snapshots()
-> Result<(), Box<dyn std::error::Error>> {
    for number in sample() {
        let model = Model {
            id: "m".into(),
            name: "m".into(),
            api: "custom".into(),
            provider: "p".into(),
            base_url: String::new(),
            reasoning: false,
            thinking_level_map: None,
            input: vec![],
            cost: ModelCost::default(),
            context_window: number,
            max_tokens: 1.0,
            headers: None,
            compat: None,
        };
        let received: Model = from_str(&to_string(&model)?)?;
        assert_eq!(received.context_window.to_bits(), number.to_bits());
        let event = AssistantMessageEvent::Start {
            partial: std::sync::Arc::new(std::sync::RwLock::new(assistant(number))),
        };
        let received: AssistantMessageEvent = from_str(&to_string(&event)?)?;
        let AssistantMessageEvent::Start { partial } = received else {
            panic!("stream tag changed")
        };
        assert_eq!(
            partial.read().unwrap().timestamp.to_bits(),
            number.to_bits()
        );
        let diagnostic = AssistantMessageDiagnostic {
            r#type: "custom".into(),
            timestamp: number,
            error: None,
            details: None,
        };
        let received: AssistantMessageDiagnostic = from_str(&to_string(&diagnostic)?)?;
        assert_eq!(received.timestamp.to_bits(), number.to_bits());
    }
    Ok(())
}

#[test]
/// Uses null for ordinary host nonfinite serialization.
fn plain_json_serializes_host_nonfinite_numbers_to_null() -> Result<(), Box<dyn std::error::Error>>
{
    for number in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        assert_eq!(to_string(&number)?, "null");
        let wire = to_string(&UserMessage {
            content: UserContent::Text("x".into()),
            timestamp: number,
        })?;
        assert!(wire.ends_with("\"timestamp\":null}"));
        assert!(from_str::<UserMessage>(&wire).is_err());
    }
    Ok(())
}

#[test]
/// Preserves the existing ordinary and nested option semantics.
fn optional_fields_keep_the_host_missing_null_rules() -> Result<(), Box<dyn std::error::Error>> {
    let missing: TextContent = from_str(r#"{"type":"text","text":"x"}"#)?;
    let null: TextContent = from_str(r#"{"type":"text","text":"x","textSignature":null}"#)?;
    assert_eq!(missing, null);
    assert_eq!(to_string(&missing)?, to_string(&null)?);
    for (text, expected) in [
        (r#"{"by":"x"}"#, None),
        (r#"{"by":"x","partition":null}"#, Some(None)),
    ] {
        let value: RoutingSort = from_str(text)?;
        assert!(matches!(value, RoutingSort::Fields { partition, .. } if partition == expected));
    }
    Ok(())
}
