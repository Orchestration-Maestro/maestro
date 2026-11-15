//! Full recorded eager-operation observations through the real dispatch owner.
#![cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::*;
#[cfg(not(target_arch = "wasm32"))]
use serde::Deserialize;
#[cfg(not(target_arch = "wasm32"))]
use serde_json::{Value, json};
#[cfg(not(target_arch = "wasm32"))]
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};

#[cfg(not(target_arch = "wasm32"))]
/// Closed input dimensions for an eager public operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// One of the four public dispatch surfaces.
    operation: String,
    /// Original optional record shape.
    presence: Option<String>,
    /// Setup or lookup failure.
    failure: Option<String>,
}
#[cfg(not(target_arch = "wasm32"))]
/// Independent callback observations and retained event stream.
#[derive(Default)]
struct Observed {
    /// Callback count before awaiting.
    calls: usize,
    /// Original callback data address, excluding trait-object metadata.
    payload_address: usize,
    /// Original carrier shape, inspected by the callback itself.
    original: bool,
    /// Event observation independent of completion's result.
    stream: Option<AssistantMessageEventStream>,
}
#[cfg(not(target_arch = "wasm32"))]
/// Controlled synchronous callback supplies one distinguishable message.
struct Provider {
    /// Original input shape to discriminate absent from supplied-empty.
    presence: Option<String>,
    /// Observations are closed before completion is awaited.
    observed: Arc<Mutex<Observed>>,
}
#[cfg(not(target_arch = "wasm32"))]
impl<E: ProviderExecution, O: Into<StreamOptionsInput<E>>> Invoke<E, ProviderCall<E, O>>
    for Provider
{
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, input: ProviderCall<E, O>) -> Self::Return {
        let common = input.options.map(Into::into).map(|options| match options {
            StreamOptionsInput::Raw(options) => options.common,
            StreamOptionsInput::Simple(options) => options.common,
        });
        let original = match (self.presence.as_deref(), common) {
            (Some("absent") | None, None) => true,
            (Some("empty"), Some(common)) => {
                common.on_payload.is_none()
                    && common.session_id.is_none()
                    && common.metadata.is_none()
            }
            (Some("full"), Some(common)) => {
                common.session_id.as_deref() == Some("retained")
                    && common.metadata == Some([("marker".into(), json!(7))].into_iter().collect())
                    && common.on_payload.as_ref().is_some_and(|hook| {
                        address(&**hook) == self.observed.lock().unwrap().payload_address
                    })
            }
            _ => false,
        };
        {
            let mut observed = self.observed.lock().unwrap();
            observed.calls += 1;
            observed.original = original;
        }
        if input.model.id == "setup" {
            return Err(DiagnosticErrorInfo::error("setup stopped"));
        }
        let stream = AssistantMessageEventStream::new();
        stream.push(AssistantMessageEvent::Done {
            reason: DoneReason::Stop,
            message: Arc::new(std::sync::RwLock::new(faux_assistant_message(
                "selected",
                FauxAssistantMessageOptions::default(),
            ))),
        });
        self.observed.lock().unwrap().stream = Some(stream.clone());
        Ok(stream)
    }
}
#[cfg(not(target_arch = "wasm32"))]
/// Compare callback data addresses without trait-object vtable metadata.
fn address<T: ?Sized>(value: &T) -> usize {
    std::ptr::from_ref(value).cast::<()>() as usize
}
#[cfg(not(target_arch = "wasm32"))]
/// Concrete closure coercion is a family representation concern.
trait OptionsFamily: ProviderExecution {
    /// Retain a source-shaped original callback and common settings.
    fn full() -> StreamOptions<Self>;
}
#[cfg(not(target_arch = "wasm32"))]
/// The same callback body has only its owning-pointer representation changed.
macro_rules! original_options {
    ($mode:ty, $pointer:ident) => {
        impl OptionsFamily for $mode {
            fn full() -> StreamOptions<Self> {
                StreamOptions {
                    on_payload: Some($pointer::new(|payload, _| Box::pin(async { Ok(payload) }))),
                    session_id: Some("retained".into()),
                    metadata: Some([("marker".into(), json!(7))].into_iter().collect()),
                    ..Default::default()
                }
            }
        }
    };
}
#[cfg(not(target_arch = "wasm32"))]
original_options!(Local, Rc);
#[cfg(not(target_arch = "wasm32"))]
original_options!(Threaded, Arc);
#[cfg(not(target_arch = "wasm32"))]
/// Retain the original callback into either carrier without reconstructing it.
fn options<E: OptionsFamily>(
    presence: Option<&str>,
    observed: &Mutex<Observed>,
) -> (
    Option<ProviderStreamOptions<E>>,
    Option<SimpleStreamOptions<E>>,
) {
    let common = match presence {
        Some("full") => E::full(),
        Some("empty") => StreamOptions::default(),
        Some("absent") | None => return (None, None),
        _ => panic!("unknown original options shape"),
    };
    observed.lock().unwrap().payload_address = common
        .on_payload
        .as_ref()
        .map_or(0, |hook| address(&**hook));
    (
        Some(ProviderStreamOptions {
            common: common.clone(),
            ..Default::default()
        }),
        Some(SimpleStreamOptions {
            common,
            ..Default::default()
        }),
    )
}
#[cfg(not(target_arch = "wasm32"))]
/// Keep immediate errors separate from result-future errors, without polling either early.
macro_rules! invoke {
    ($input:ident, $model:ident, $context:ident, $observed:ident, $mode:ty,
     $raw:path, $simple:path, $complete:path, $complete_simple:path $(, $spawn:ident)?) => {{
        let (raw, simple) = options::<$mode>($input.presence.as_deref(), $observed);
        match $input.operation.as_str() {
            "stream" => inspect_stream($raw($model, $context, raw $(, $spawn)?), $observed).await,
            "streamSimple" => inspect_stream($simple($model, $context, simple $(, $spawn)?), $observed).await,
            "complete" => {
                let result = $complete($model, $context, raw $(, $spawn)?);
                let before = $observed.lock().unwrap().calls;
                inspect_completion(result.await, $observed, before).await
            }
            "completeSimple" => {
                let result = $complete_simple($model, $context, simple $(, $spawn)?);
                let before = $observed.lock().unwrap().calls;
                inspect_completion(result.await, $observed, before).await
            }
            _ => panic!("unknown public operation"),
        }
    }};
}
#[cfg(not(target_arch = "wasm32"))]
/// Every recorded field is observed independently after its corresponding operation.
#[test]
fn recorded_dispatch_corpus_keeps_all_fields() -> crate::TestResult {
    let _serial = crate::bundled::registry();
    crate::runtime::run(async {
        for row in super::corpus::load().dispatch.rows {
            if row.input.get("outcome").is_some() {
                continue;
            }
            let input: Input = serde_json::from_value(row.input)?;
            for family in ["local", "threaded", "default"] {
                let observed = Arc::new(Mutex::new(Observed::default()));
                register(&input, &observed);
                let actual = run(&input, &observed, family).await?;
                assert_eq!(actual, row.expected, "{} {family}", row.name);
            }
        }
        Ok(())
    })
}
#[cfg(not(target_arch = "wasm32"))]
/// Call the selected family without running the callback in a replacement registry.
async fn run(
    input: &Input,
    observed: &Arc<Mutex<Observed>>,
    family: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let mut model = crate::bundled::descriptor("controlled", "https://fixture.invalid")?;
    model.id = input.failure.clone().unwrap_or_else(|| "model".into());
    let context = crate::bundled::conversation()?;
    match family {
        "local" => run_local(input, observed, model, context).await,
        "threaded" => run_threaded(input, observed, model, context).await,
        "default" => run_default(input, observed, model, context).await,
        _ => unreachable!("known family"),
    }
}
#[cfg(not(target_arch = "wasm32"))]
/// Generate only each public entry's concrete type/arity selection.
macro_rules! runner {
    ($name:ident, $mode:ty, $raw:path, $simple:path, $complete:path, $complete_simple:path $(, $pointer:ident)?) => {
        /// Invoke all four operations with the selected entry's original signature.
        async fn $name(input: &Input, observed: &Mutex<Observed>, model: Model, context: Context)
            -> Result<Value, Box<dyn std::error::Error>> {
            $(let spawn: ProviderSpawner<$mode> = $pointer::new(|_| panic!("synchronous callback must not submit"));)?
            runner!(@invoke input, model, context, observed, $mode, $raw, $simple, $complete, $complete_simple, spawn $(, $pointer)? )
        }
    };
    (@invoke $input:ident, $model:ident, $context:ident, $observed:ident, $mode:ty,
        $raw:path, $simple:path, $complete:path, $complete_simple:path, $spawn:ident, $pointer:ident) => {
        invoke!($input, $model, $context, $observed, $mode, $raw, $simple, $complete, $complete_simple, $spawn)
    };
    (@invoke $input:ident, $model:ident, $context:ident, $observed:ident, $mode:ty,
        $raw:path, $simple:path, $complete:path, $complete_simple:path, $spawn:ident) => {
        invoke!($input, $model, $context, $observed, $mode, $raw, $simple, $complete, $complete_simple)
    };
}
#[cfg(not(target_arch = "wasm32"))]
runner!(
    run_local,
    Local,
    stream_for::<Local>,
    stream_simple_for::<Local>,
    complete_for::<Local>,
    complete_simple_for::<Local>,
    Rc
);
#[cfg(not(target_arch = "wasm32"))]
runner!(
    run_threaded,
    Threaded,
    stream_for::<Threaded>,
    stream_simple_for::<Threaded>,
    complete_for::<Threaded>,
    complete_simple_for::<Threaded>,
    Arc
);
#[cfg(not(target_arch = "wasm32"))]
runner!(
    run_default,
    Threaded,
    stream,
    stream_simple,
    complete,
    complete_simple
);
#[cfg(not(target_arch = "wasm32"))]
/// Inspect stream setup before its first observation future exists.
async fn inspect_stream(
    result: Result<AssistantMessageEventStream, DiagnosticErrorInfo>,
    observed: &Mutex<Observed>,
) -> Result<Value, Box<dyn std::error::Error>> {
    match result {
        Err(error) => Ok(json!({"phase":"immediate", "error": error.message})),
        Ok(stream) => {
            let (before, original) = {
                let seen = observed.lock().unwrap();
                (seen.calls, seen.original)
            };
            let (events, message) = crate::transport::drain(&stream).await?;
            success(before, original, &events, &message)
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
/// Result observation does not consume the stream retained by the supplied callback.
async fn inspect_completion(
    result: Result<SharedAssistantMessage, DiagnosticErrorInfo>,
    observed: &Mutex<Observed>,
    before: usize,
) -> Result<Value, Box<dyn std::error::Error>> {
    match result {
        Err(error) => Ok(json!({"phase":"returned-future", "error": error.message})),
        Ok(message) => {
            let (original, stream) = {
                let seen = observed.lock().unwrap();
                (seen.original, seen.stream.clone().unwrap())
            };
            assert!(Arc::ptr_eq(&message, &stream.result().await));
            let (events, result) = crate::transport::drain(&stream).await?;
            success(before, original, &events, &result)
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
/// Publish actual text, not a label inferred from callback presence.
fn success(
    before: usize,
    original: bool,
    events: &[String],
    message: &AssistantMessage,
) -> Result<Value, Box<dyn std::error::Error>> {
    let [AssistantContent::Text(text)] = message.content.as_slice() else {
        return Err("one text block".into());
    };
    Ok(
        json!({"callsBeforeAwait":before,"originalOptions":original,"text":text.text,"events":events}),
    )
}

#[cfg(not(target_arch = "wasm32"))]
/// Missing-provider inputs clear the real registry; other inputs install one inert owner.
fn register(input: &Input, observed: &Arc<Mutex<Observed>>) {
    clear_api_providers();
    let owner = Arc::new(Provider {
        presence: input.presence.clone(),
        observed: observed.clone(),
    });
    if input.failure.as_deref() != Some("missing") {
        register_api_provider(
            ApiProvider {
                api: "controlled".into(),
                stream: owner.clone(),
                stream_simple: owner,
            },
            None,
        );
    }
}
