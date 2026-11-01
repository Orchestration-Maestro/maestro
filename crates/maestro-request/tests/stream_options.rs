//! Shared invocation options retain data without requiring executable defaults.
use maestro_request::types::{
    CacheRetention, ProviderResponse, ProviderStreamOptions, SimpleStreamOptions,
    StreamOptionSlots, StreamOptions, StreamOptionsInput, ThinkingBudgets, ThinkingLevel,
    Transport,
};
use serde_json::json;
use std::rc::Rc;

/// A family without a default implementation or serializable capabilities.
#[derive(Clone)]
struct Slots;
impl StreamOptionSlots for Slots {
    type Signal = Rc<dyn Fn()>;
    type PayloadHook = Rc<dyn Fn()>;
    type ResponseHook = Rc<dyn Fn()>;
    type Fetch = Rc<dyn Fn()>;
}

#[test]
fn stream_records_keep_every_common_option() {
    common_defaults();
    for number in [None, Some(0.0_f64), Some(-0.0), Some(1.25)] {
        let callback: Rc<dyn Fn()> = Rc::new(|| {});
        let common = common_options(number, &callback);
        common_data(&common, number);
        common_owners(common.clone(), &callback);
        thinking_budgets(number);
        option_forms(common, &callback, number);
    }
    wire_records();
}

/// Defaults require neither a family default nor executable capability values.
fn common_defaults() {
    let common = StreamOptions::<Slots>::default();
    assert!(common.temperature.is_none() && common.max_tokens.is_none());
    assert!(common.signal.is_none() && common.api_key.is_none());
    assert!(common.transport.is_none() && common.cache_retention.is_none());
    assert!(common.session_id.is_none() && common.on_payload.is_none());
    assert!(common.on_response.is_none() && common.headers.is_none());
    assert!(common.timeout_ms.is_none() && common.max_retries.is_none());
    assert!(common.max_retry_delay_ms.is_none() && common.metadata.is_none());
    assert!(common.fetch.is_none());
}

/// Supplies every common field with distinguishable data and shared capabilities.
fn common_options(number: Option<f64>, callback: &Rc<dyn Fn()>) -> StreamOptions<Slots> {
    StreamOptions {
        base_url: None,
        temperature: number,
        max_tokens: number,
        timeout_ms: number,
        max_retries: number,
        max_retry_delay_ms: number,
        transport: Some(Transport::WebsocketCached),
        cache_retention: Some(CacheRetention::Long),
        api_key: Some("key".into()),
        session_id: Some("session".into()),
        headers: Some([("z".into(), "last".into()), ("a".into(), "first".into())].into()),
        metadata: Some(json!({"sentinel": false}).as_object().unwrap().clone()),
        signal: Some(callback.clone()),
        on_payload: Some(callback.clone()),
        on_response: Some(callback.clone()),
        fetch: Some(callback.clone()),
    }
}

/// Serialization retains every data field while omitting all capability representations.
fn common_data(common: &StreamOptions<Slots>, number: Option<f64>) {
    let data = serde_json::to_value(common).unwrap();
    assert_eq!(data["transport"], "websocket-cached");
    assert_eq!(data["cacheRetention"], "long");
    assert_eq!(data["apiKey"], "key");
    assert_eq!(data["sessionId"], "session");
    assert_eq!(data["headers"], json!({"z":"last", "a":"first"}));
    assert_eq!(data["metadata"], json!({"sentinel":false}));
    for capability in ["signal", "onPayload", "onResponse", "fetch"] {
        assert!(!data.as_object().unwrap().contains_key(capability));
    }
    let decoded: StreamOptions<Slots> = serde_json::from_value(data).unwrap();
    for field in [
        decoded.temperature,
        decoded.max_tokens,
        decoded.timeout_ms,
        decoded.max_retries,
        decoded.max_retry_delay_ms,
    ] {
        assert_eq!(field.map(f64::to_bits), number.map(f64::to_bits));
    }
    assert_eq!(decoded.transport, Some(Transport::WebsocketCached));
    assert_eq!(decoded.cache_retention, Some(CacheRetention::Long));
    assert!(decoded.signal.is_none() && decoded.on_payload.is_none());
    assert!(decoded.on_response.is_none() && decoded.fetch.is_none());
}

/// Cloning shares capabilities while retaining numbers and ordered headers exactly.
fn common_owners(copy: StreamOptions<Slots>, callback: &Rc<dyn Fn()>) {
    assert_eq!(copy.api_key.as_deref(), Some("key"));
    assert_eq!(copy.session_id.as_deref(), Some("session"));
    assert_eq!(
        copy.headers
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["z", "a"]
    );
    assert_eq!(copy.metadata.unwrap()["sentinel"], false);
    for field in [copy.signal, copy.on_payload, copy.on_response, copy.fetch] {
        assert!(Rc::ptr_eq(field.as_ref().unwrap(), callback));
    }
}

/// All reasoning budgets retain absence and exact finite numeric values.
fn thinking_budgets(number: Option<f64>) {
    let budgets = ThinkingBudgets {
        minimal: number,
        low: number,
        medium: number,
        high: number,
    };
    let decoded: ThinkingBudgets =
        serde_json::from_str(&serde_json::to_string(&budgets).unwrap()).unwrap();
    for field in [decoded.minimal, decoded.low, decoded.medium, decoded.high] {
        assert_eq!(field.map(f64::to_bits), number.map(f64::to_bits));
    }
}

/// Raw and simple forms retain their own extra data and executable ownership.
fn option_forms(common: StreamOptions<Slots>, callback: &Rc<dyn Fn()>, number: Option<f64>) {
    let mut raw = ProviderStreamOptions::<_, Option<Rc<dyn Fn()>>> {
        common,
        objects: Some(callback.clone()),
        ..Default::default()
    };
    raw.extra.insert("open".into(), json!([false, 0, null]));
    let copied: StreamOptionsInput<_, _, Rc<dyn Fn()>> = raw.clone().into();
    assert!(Rc::ptr_eq(
        copied.common().on_payload.as_ref().unwrap(),
        callback
    ));
    let StreamOptionsInput::Raw(copied) = copied.clone() else {
        panic!("raw options changed variant");
    };
    assert_eq!(copied.extra["open"], json!([false, 0, null]));
    assert!(Rc::ptr_eq(copied.objects.as_ref().unwrap(), callback));
    let budgets = ThinkingBudgets {
        minimal: number,
        low: number,
        medium: number,
        high: number,
    };
    let simple = SimpleStreamOptions::<_, Rc<dyn Fn()>> {
        common: raw.common,
        reasoning: Some(ThinkingLevel::High),
        thinking_budgets: Some(budgets.clone()),
        tool_choice: Some(callback.clone()),
    };
    let copied: StreamOptionsInput<_, Option<Rc<dyn Fn()>>, _> = simple.clone().into();
    assert!(Rc::ptr_eq(
        copied.common().on_response.as_ref().unwrap(),
        callback
    ));
    let StreamOptionsInput::Simple(copied) = copied.clone() else {
        panic!("simple options changed variant");
    };
    assert_eq!(copied.reasoning, Some(ThinkingLevel::High));
    assert_eq!(copied.thinking_budgets, Some(budgets));
    assert!(Rc::ptr_eq(simple.tool_choice.as_ref().unwrap(), callback));
    let defaults = SimpleStreamOptions::<StreamOptions<Slots>, Rc<dyn Fn()>>::default();
    assert!(
        defaults.reasoning.is_none()
            && defaults.thinking_budgets.is_none()
            && defaults.tool_choice.is_none()
    );
}

/// Enum wire spellings and copied response headers remain unchanged.
fn wire_records() {
    for (transport, wire) in [
        (Transport::Sse, "sse"),
        (Transport::Websocket, "websocket"),
        (Transport::WebsocketCached, "websocket-cached"),
        (Transport::Auto, "auto"),
    ] {
        assert_eq!(serde_json::to_value(&transport).unwrap(), wire);
        assert_eq!(
            serde_json::from_value::<Transport>(json!(wire)).unwrap(),
            transport
        );
    }
    for (cache, wire) in [
        (CacheRetention::None, "none"),
        (CacheRetention::Short, "short"),
        (CacheRetention::Long, "long"),
    ] {
        assert_eq!(serde_json::to_value(cache).unwrap(), wire);
        assert_eq!(
            serde_json::from_value::<CacheRetention>(json!(wire)).unwrap(),
            cache
        );
    }
    assert_eq!(
        ThinkingBudgets::default(),
        ThinkingBudgets {
            minimal: None,
            low: None,
            medium: None,
            high: None
        }
    );
    let response = ProviderResponse {
        status: 207.0,
        headers: [("x-source".into(), "native".into())].into(),
    };
    assert_eq!(
        serde_json::to_value(&response).unwrap(),
        json!({"status":207.0,"headers":{"x-source":"native"}})
    );
    assert_eq!(
        serde_json::from_value::<ProviderResponse>(serde_json::to_value(&response).unwrap())
            .unwrap(),
        response
    );
}
