//! Recorded ordered registry changes retain their original callback owners.
#![cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
use crate::bundled::registered;
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::*;
#[cfg(not(target_arch = "wasm32"))]
use serde::Deserialize;
#[cfg(not(target_arch = "wasm32"))]
use serde_json::{Value, json};
#[cfg(not(target_arch = "wasm32"))]
use std::{
    rc::Rc,
    sync::{Arc, RwLock},
};

#[cfg(not(target_arch = "wasm32"))]
/// Closed, operation-specific registry inputs.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Input {
    /// Ordered installation followed by an in-place replacement.
    Replace { keys: Vec<String>, replaced: String },
    /// Source-matched cleanup.
    Cleanup { remove: String },
    /// Explicit registry clear.
    Clear { clear: bool },
    /// Checked callback identity mismatch.
    Guard {
        form: String,
        supplied: String,
        registered: String,
    },
}
#[cfg(not(target_arch = "wasm32"))]
/// A visible callback label distinguishes retained from replacement ownership.
struct Label(&'static str);
#[cfg(not(target_arch = "wasm32"))]
impl<E: ProviderExecution, O> Invoke<E, ProviderCall<E, O>> for Label {
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, _input: ProviderCall<E, O>) -> Self::Return {
        let stream = AssistantMessageEventStream::new();
        stream.push(AssistantMessageEvent::Done {
            reason: DoneReason::Stop,
            message: Arc::new(RwLock::new(faux_assistant_message(
                self.0,
                FauxAssistantMessageOptions::default(),
            ))),
        });
        Ok(stream)
    }
}
#[cfg(not(target_arch = "wasm32"))]
/// Register one owner into both callback slots.
fn register(api: &str, label: &'static str, source: &str) {
    let callback = Arc::new(Label(label));
    register_api_provider(
        ApiProvider {
            api: api.into(),
            stream: callback.clone(),
            stream_simple: callback,
        },
        Some(source.into()),
    );
}
#[cfg(not(target_arch = "wasm32"))]
/// Exercise the five sequential source observations in each family.
#[test]
fn recorded_registry_corpus_keeps_all_fields() -> crate::TestResult {
    let _serial = crate::bundled::registry();
    crate::runtime::run(async {
        for local in [true, false] {
            clear_api_providers();
            let mut retained = None;
            for row in super::corpus::load()
                .registered
                .rows
                .into_iter()
                .filter(|row| !row.name.starts_with("registered_"))
            {
                let input: Input = serde_json::from_value(row.input)?;
                let actual = observe(input, &mut retained, local).await?;
                assert_eq!(actual, row.expected, "{} local={local}", row.name);
            }
        }
        Ok(())
    })
}
#[cfg(not(target_arch = "wasm32"))]
/// Keep a surviving callback through replacement, source cleanup and clear.
async fn observe(
    input: Input,
    retained: &mut Option<ApiProvider>,
    local: bool,
) -> Result<Value, Box<dyn std::error::Error>> {
    match input {
        Input::Replace { keys, replaced } => {
            for key in &keys {
                register(
                    key,
                    if key == &replaced {
                        "retained"
                    } else {
                        "other"
                    },
                    "old",
                );
            }
            *retained = get_api_provider(&replaced);
            register(&replaced, "replacement", "new");
            let old = retained.as_ref().unwrap();
            let current = get_api_provider(&replaced).unwrap();
            Ok(
                json!({"order": registered(), "retained": text(&old.stream, &replaced, local).await?,
                "current": text(&current.stream, &replaced, local).await?}),
            )
        }
        Input::Cleanup { remove } => {
            unregister_api_providers(&remove);
            Ok(json!({"remaining":registered()}))
        }
        Input::Clear { clear } => {
            assert!(clear);
            clear_api_providers();
            let old = retained.as_ref().unwrap();
            Ok(json!({"registrySize": get_api_providers().len(),
                "retained": text(&old.stream_simple, &old.api, local).await?}))
        }
        Input::Guard {
            form,
            supplied,
            registered,
        } => {
            let old = retained.as_ref().unwrap();
            assert_eq!(old.api, registered);
            let error = guard(old, &form, &supplied, local);
            Ok(json!({"error":error.message}))
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
/// Choose only the family envelope; synchronous callbacks never submit production.
fn invocation<K: StreamOptionsFamily>(local: bool) -> ProviderInvocation<K> {
    if local {
        ProviderInvocation::Local(None, Rc::new(|_| panic!("label must not submit")))
    } else {
        ProviderInvocation::Threaded(None, Some(Arc::new(|_| panic!("label must not submit"))))
    }
}
#[cfg(not(target_arch = "wasm32"))]
/// Observe the actual terminal body through either retained callback type.
async fn text<K: StreamOptionsFamily>(
    callback: &StreamFunction<K>,
    api: &str,
    local: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let model = crate::bundled::descriptor(api, "https://fixture.invalid")?;
    let stream = callback.call(
        model,
        crate::bundled::conversation()?,
        invocation::<K>(local),
    )?;
    let (_, message) = crate::transport::drain(&stream).await?;
    let [AssistantContent::Text(text)] = message.content.as_slice() else {
        panic!("one callback label")
    };
    Ok(text.text.clone())
}
#[cfg(not(target_arch = "wasm32"))]
/// Checked handles reject a mismatched model after removal, before invoking caller code.
fn guard(old: &ApiProvider, form: &str, supplied: &str, local: bool) -> DiagnosticErrorInfo {
    let model = crate::bundled::descriptor(supplied, "https://fixture.invalid").unwrap();
    let context = crate::bundled::conversation().unwrap();
    let result = match form {
        "stream" => old
            .stream
            .call(model, context, invocation::<ProviderStreamOptions>(local)),
        "streamSimple" => {
            old.stream_simple
                .call(model, context, invocation::<SimpleStreamOptions>(local))
        }
        _ => panic!("unknown guarded operation"),
    };
    result.err().unwrap()
}
