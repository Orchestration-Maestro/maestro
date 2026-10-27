//! SDK constructors retain execution-family inference through the shared option owner.
use maestro_models::{
    Local, OnPayload, OnResponse, ProviderStreamOptions, SimpleStreamOptions, StreamOptions,
    StreamOptionsInput, Threaded, ToolChoice,
};
use std::{rc::Rc, sync::Arc};

#[test]
fn native_stream_aliases_keep_sdk_constructor_inference() {
    let threaded: StreamOptions = StreamOptions {
        on_payload: Some(Arc::new(|input, _| Box::pin(async move { Ok(input) }))),
        on_response: Some(Arc::new(|_, _| Box::pin(async { Ok(()) }))),
        ..Default::default()
    };
    let _: &Option<OnPayload<Threaded>> = &threaded.on_payload;
    let _: &Option<OnResponse<Threaded>> = &threaded.on_response;
    let local: StreamOptions<Local> = StreamOptions {
        on_payload: Some(Rc::new(|input, _| Box::pin(async move { Ok(input) }))),
        on_response: Some(Rc::new(|_, _| Box::pin(async { Ok(()) }))),
        ..Default::default()
    };
    let _: &Option<OnPayload<Local>> = &local.on_payload;
    let _: &Option<OnResponse<Local>> = &local.on_response;
    let mut raw: ProviderStreamOptions = ProviderStreamOptions {
        common: threaded,
        ..Default::default()
    };
    raw.objects.insert(42_u32);
    raw.extra.insert("open".into(), serde_json::json!(false));
    assert_eq!(raw.clone().objects.get::<u32>(), Some(&42));
    let simple: SimpleStreamOptions<Local> = SimpleStreamOptions {
        common: local,
        tool_choice: Some(ToolChoice::None),
        ..Default::default()
    };
    let defaults: SimpleStreamOptions = SimpleStreamOptions::default();
    assert!(defaults.tool_choice.is_none());
    let raw: maestro_request::types::StreamOptionsInput<StreamOptions, _, ToolChoice> =
        StreamOptionsInput::from(raw);
    assert!(raw.common().on_payload.is_some());
    assert!(
        matches!(raw, StreamOptionsInput::Raw(value) if value.extra["open"] == false && value.objects.get::<u32>() == Some(&42))
    );
    let simple: maestro_request::types::StreamOptionsInput<
        StreamOptions<Local>,
        maestro_models::ProviderObjects,
        _,
    > = StreamOptionsInput::from(simple);
    assert!(simple.common().on_response.is_some());
    assert!(
        matches!(simple, StreamOptionsInput::Simple(value) if value.tool_choice == Some(ToolChoice::None))
    );
}
