//! Original option carriers through either public provider writer.
#![cfg(test)]
use maestro_models::*;
use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
};

/// One expected call, retaining only identities independently supplied by the caller.
pub struct Expected {
    /// Absent, supplied-empty or supplied-full.
    row: usize,
    /// The original raw versus simple surface.
    simple: bool,
    /// Original hook addresses, independent of trait object metadata.
    hooks: [usize; 2],
    /// Original cooperative signal.
    signal: Option<Cancellation>,
    /// Original injected object.
    object: Arc<u32>,
    /// Original HTTP adapter.
    fetch: Fetch,
}
/// One owner installed into both callback slots, also usable by the catalog writer.
#[derive(Default)]
pub struct OptionsProvider {
    /// Expectations are removed before submitting any caller work.
    expected: Mutex<VecDeque<Expected>>,
    /// Closed, ordered observations after original options survive suspension.
    pub observed: Mutex<Vec<String>>,
}
/// Erase futures only at the concrete family boundary.
pub trait Produce: ProviderExecution {
    /// Keep the original call until the producer resumes.
    fn produce(
        call: ProviderCall<Self, StreamOptionsInput<Self>>,
        expected: Expected,
    ) -> Self::Future<'static, ()>;
}
/// The same producer checks both associated future representations.
macro_rules! produce {
    ($mode:ty) => {
        impl Produce for $mode {
            fn produce(
                call: ProviderCall<Self, StreamOptionsInput<Self>>,
                expected: Expected,
            ) -> Self::Future<'static, ()> {
                Box::pin(async move {
                    tokio::task::yield_now().await;
                    check(call, expected).await;
                })
            }
        }
    };
}
produce!(Local);
produce!(Threaded);
impl<E: Produce, O: Into<StreamOptionsInput<E>>> Invoke<E, ProviderCall<E, O>> for OptionsProvider {
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, call: ProviderCall<E, O>) -> Self::Return {
        let expected = self.expected.lock().unwrap().pop_front().unwrap();
        self.observed
            .lock()
            .unwrap()
            .push(format!("{}:{}", expected.simple, expected.row));
        let submit = call.submit.clone();
        let input = ProviderCall {
            model: call.model,
            context: call.context,
            options: call.options.map(Into::into),
            submit: call.submit,
        };
        E::submit(E::produce(input, expected), submit, |_| {
            panic!("supplied admission required")
        })?;
        let stream = AssistantMessageEventStream::new();
        stream.push(AssistantMessageEvent::Done {
            reason: DoneReason::Stop,
            message: Arc::new(RwLock::new(faux_assistant_message(
                "original",
                FauxAssistantMessageOptions::default(),
            ))),
        });
        Ok(stream)
    }
}
/// Check the original variant and every original field after callback return.
async fn check<E: ProviderExecution>(
    call: ProviderCall<E, StreamOptionsInput<E>>,
    expected: Expected,
) {
    assert_eq!(call.context.system_prompt.as_deref(), Some("instruction"));
    let Some(options) = call.options else {
        assert_eq!(expected.row, 0);
        return;
    };
    assert_ne!(expected.row, 0);
    let common = check_variant(options, &expected);
    check_values(&common, expected.row == 2);
    if expected.row == 1 {
        assert!(
            common.signal.is_none()
                && common.on_payload.is_none()
                && common.on_response.is_none()
                && common.fetch.is_none()
        );
        return;
    }
    let payload = common.on_payload.unwrap();
    let response = common.on_response.unwrap();
    assert_eq!(
        std::ptr::from_ref(&*payload).cast::<()>() as usize,
        expected.hooks[0]
    );
    assert_eq!(
        std::ptr::from_ref(&*response).cast::<()>() as usize,
        expected.hooks[1]
    );
    assert!(Arc::ptr_eq(common.fetch.as_ref().unwrap(), &expected.fetch));
    common.signal.as_ref().unwrap().abort();
    assert!(expected.signal.unwrap().is_aborted());
    let model = Arc::new(call.model);
    let selected = payload(
        PayloadHookInput {
            payload: serde_json::json!(11),
            signal: common.signal,
        },
        model.clone(),
    )
    .await
    .unwrap();
    assert_eq!(selected.payload, serde_json::json!(22));
    response(ProviderResponse::default(), model).await.unwrap();
}
/// Preserve raw object identity and the original simple-only settings.
fn check_variant<E: ProviderExecution>(
    options: StreamOptionsInput<E>,
    expected: &Expected,
) -> StreamOptions<E> {
    assert_eq!(
        matches!(&options, StreamOptionsInput::Simple(_)),
        expected.simple
    );
    match options {
        StreamOptionsInput::Raw(raw) => {
            check_raw(&raw, expected);
            raw.common
        }
        StreamOptionsInput::Simple(simple) => {
            if expected.row == 2 {
                assert_eq!(simple.reasoning, Some(ThinkingLevel::High));
                assert_eq!(simple.thinking_budgets, Some(budgets()));
                assert_eq!(
                    simple.tool_choice,
                    Some(ToolChoice::Function {
                        name: "lookup".into()
                    })
                );
            } else {
                assert!(simple.reasoning.is_none());
                assert!(simple.thinking_budgets.is_none());
                assert!(simple.tool_choice.is_none());
            }
            simple.common
        }
    }
}
/// Check provider-specific open fields and injected-object sharing.
fn check_raw<E: ProviderExecution>(raw: &ProviderStreamOptions<E>, expected: &Expected) {
    if expected.row == 2 {
        assert_eq!(
            raw.extra,
            serde_json::from_value::<JsonObject>(
                serde_json::json!({"custom":["z","a","z"],"reasoning":{"mode":"custom"}})
            )
            .unwrap()
        );
        assert!(Arc::ptr_eq(
            raw.objects.get::<Arc<u32>>().unwrap(),
            &expected.object
        ));
    } else {
        assert!(raw.extra.is_empty());
        assert!(raw.objects.get::<Arc<u32>>().is_none());
    }
}
/// Independent values for all serializable common fields, including present-empty neighbors.
fn check_values<E: ProviderExecution>(actual: &StreamOptions<E>, full: bool) {
    let expected = if full {
        StreamOptions::<Threaded> {
            temperature: Some(0.25),
            max_tokens: Some(17.0),
            api_key: Some("controlled".into()),
            transport: Some(Transport::WebsocketCached),
            cache_retention: Some(CacheRetention::Long),
            session_id: Some("session".into()),
            headers: Some([("x".into(), "y".into())].into()),
            timeout_ms: Some(123.0),
            max_retries: Some(2.0),
            max_retry_delay_ms: Some(456.0),
            metadata: Some(serde_json::from_value(serde_json::json!({"caller":null})).unwrap()),
            ..Default::default()
        }
    } else {
        StreamOptions::default()
    };
    assert_eq!(actual.temperature, expected.temperature);
    assert_eq!(actual.max_tokens, expected.max_tokens);
    assert_eq!(actual.api_key, expected.api_key);
    assert_eq!(actual.transport, expected.transport);
    assert_eq!(actual.cache_retention, expected.cache_retention);
    assert_eq!(actual.session_id, expected.session_id);
    assert_eq!(actual.headers, expected.headers);
    assert_eq!(actual.timeout_ms, expected.timeout_ms);
    assert_eq!(actual.max_retries, expected.max_retries);
    assert_eq!(actual.max_retry_delay_ms, expected.max_retry_delay_ms);
    assert_eq!(actual.metadata, expected.metadata);
}
/// Distinguishable budgets for every supported level.
fn budgets() -> ThinkingBudgets {
    ThinkingBudgets {
        minimal: Some(1.0),
        low: Some(2.0),
        medium: Some(3.0),
        high: Some(4.0),
    }
}
/// Install original options for a single call without invoking any hooks.
fn options<E: ProviderExecution>(
    common: StreamOptions<E>,
    row: usize,
    simple: bool,
    owner: &OptionsProvider,
) -> Option<StreamOptionsInput<E>> {
    let object = Arc::new(37);
    owner.expected.lock().unwrap().push_back(Expected {
        row,
        simple,
        hooks: [
            common
                .on_payload
                .as_ref()
                .map_or(0, |hook| std::ptr::from_ref(&**hook).cast::<()>() as usize),
            common
                .on_response
                .as_ref()
                .map_or(0, |hook| std::ptr::from_ref(&**hook).cast::<()>() as usize),
        ],
        signal: common.signal.clone(),
        object: object.clone(),
        fetch: common
            .fetch
            .clone()
            .unwrap_or_else(|| Arc::new(|_| Box::pin(async { panic!("fetch must stay inert") }))),
    });
    if row == 0 {
        return None;
    }
    Some(if simple {
        StreamOptionsInput::Simple(SimpleStreamOptions {
            common,
            reasoning: (row == 2).then_some(ThinkingLevel::High),
            thinking_budgets: (row == 2).then(budgets),
            tool_choice: (row == 2).then(|| ToolChoice::Function {
                name: "lookup".into(),
            }),
        })
    } else {
        let mut raw = ProviderStreamOptions {
            common,
            ..Default::default()
        };
        if row == 2 {
            raw.extra = serde_json::from_value(
                serde_json::json!({"custom":["z","a","z"],"reasoning":{"mode":"custom"}}),
            )
            .unwrap();
            raw.objects.insert(object);
        }
        StreamOptionsInput::Raw(raw)
    })
}
/// Call the selected original surface through the public checked callback.
fn invoke<E: ProviderExecution>(
    provider: &ApiProvider,
    model: &Model,
    options: Option<StreamOptionsInput<E>>,
    simple: bool,
    spawn: ProviderSpawner<E>,
) -> AssistantMessageEventStream {
    let context = Context {
        system_prompt: Some("instruction".into()),
        messages: vec![],
        tools: None,
    };
    if simple {
        let options = options.map(|options| {
            let StreamOptionsInput::Simple(options) = options else {
                unreachable!()
            };
            options
        });
        provider.stream_simple.call(
            model.clone(),
            context,
            E::submission(options, E::provided(spawn)),
        )
    } else {
        let options = options.map(|options| {
            let StreamOptionsInput::Raw(options) = options else {
                unreachable!()
            };
            options
        });
        provider.stream.call(
            model.clone(),
            context,
            E::submission(options, E::provided(spawn)),
        )
    }
    .unwrap()
}
/// Populate both hook families with genuine retained local or thread-safe captures.
macro_rules! family_run {
    ($name:ident, $mode:ty, $pointer:ident, $tasks:expr, $spawn:expr, $capture:expr) => {
        async fn $name(provider: &ApiProvider, model: &Model, owner: &OptionsProvider) {
            for simple in [false, true] {
                for row in 0..3 {
                    let capture = $capture;
                    let payload_capture = capture.clone();
                    let mut common = StreamOptions::<$mode>::default();
                    if row == 2 {
                        common = full_common();
                        common.on_payload = Some($pointer::new(move |mut input, _| {
                            let capture = payload_capture.clone();
                            Box::pin(async move {
                                tokio::task::yield_now().await;
                                assert_eq!(*capture, 37);
                                assert!(input.signal.as_ref().unwrap().is_aborted());
                                input.payload = serde_json::json!(22);
                                Ok(input)
                            })
                        }));
                        common.on_response = Some($pointer::new(move |_, _| {
                            let capture = capture.clone();
                            Box::pin(async move {
                                tokio::task::yield_now().await;
                                assert_eq!(*capture, 37);
                                Ok(())
                            })
                        }));
                    }
                    let options = options(common, row, simple, owner);
                    let tasks = $tasks;
                    let spawn: ProviderSpawner<$mode> = ($spawn)(&tasks);
                    let invocation = invoke::<$mode>(provider, model, options, simple, spawn);
                    let result = invocation.result().await;
                    assert_eq!(
                        result.read().unwrap().content,
                        [AssistantContent::Text(faux_text("original"))]
                    );
                    for task in tasks.take() {
                        task.await.unwrap();
                    }
                }
            }
        }
    };
}
family_run!(
    local,
    Local,
    Rc,
    Rc::new(RefCell::new(Vec::new())),
    |tasks: &Rc<RefCell<Vec<tokio::task::JoinHandle<()>>>>| {
        let tasks = tasks.clone();
        Rc::new(move |task| {
            let task = tokio::task::spawn_local(task);
            tasks.borrow_mut().push(task);
            true
        })
    },
    Rc::new(37)
);
family_run!(
    threaded,
    Threaded,
    Arc,
    Rc::new(RefCell::new(Vec::new())),
    |tasks: &Rc<RefCell<Vec<tokio::task::JoinHandle<()>>>>| {
        // Admission is thread-safe; the caller retains the joined task through a channel.
        let (send, receive) = std::sync::mpsc::channel();
        let spawn = Arc::new(move |task| {
            send.send(tokio::spawn(task)).unwrap();
            true
        });
        let receive_task = tokio::task::spawn_local(async move {
            receive.recv().unwrap().await.unwrap();
        });
        tasks.borrow_mut().push(receive_task);
        spawn
    },
    Arc::new(37)
);
/// A complete caller record with controls distinct from the protocol defaults.
fn full_common<E: ProviderExecution>() -> StreamOptions<E> {
    StreamOptions {
        temperature: Some(0.25),
        max_tokens: Some(17.0),
        signal: Some(Cancellation::new()),
        api_key: Some("controlled".into()),
        transport: Some(Transport::WebsocketCached),
        cache_retention: Some(CacheRetention::Long),
        session_id: Some("session".into()),
        headers: Some([("x".into(), "y".into())].into()),
        timeout_ms: Some(123.0),
        max_retries: Some(2.0),
        max_retry_delay_ms: Some(456.0),
        metadata: Some(serde_json::from_value(serde_json::json!({"caller":null})).unwrap()),
        fetch: Some(Arc::new(|_| {
            Box::pin(async { panic!("fetch must stay inert") })
        })),
        ..Default::default()
    }
}
/// Complete every raw/simple and absent/empty/full case through the supplied retained provider.
pub async fn exercise(provider: &ApiProvider, model: &Model, owner: &OptionsProvider) {
    let before = owner.observed.lock().unwrap().len();
    local(provider, model, owner).await;
    threaded(provider, model, owner).await;
    assert_eq!(
        &owner.observed.lock().unwrap()[before..],
        [
            "false:0", "false:1", "false:2", "true:0", "true:1", "true:2", "false:0", "false:1",
            "false:2", "true:0", "true:1", "true:2"
        ]
    );
    assert!(owner.expected.lock().unwrap().is_empty());
}
