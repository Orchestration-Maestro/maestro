//! Native registered providers await the compiled guest's original hooks.
#![cfg(test)]
use maestro_models::*;
use serde_json::{Value, json};
use std::sync::{Arc, RwLock};

/// Erase the controlled producer only at each existing execution-family boundary.
trait Produce: ProviderExecution {
    /// Read the selected payload at the next native consumer and emit one terminal result.
    fn produce(
        common: StreamOptions<Self>,
        model: Model,
        stream: AssistantMessageEventStream,
    ) -> Self::Future<'static, ()>;
}
/// Both registration families enter the same native consuming phase.
macro_rules! producer {
    ($mode:ty) => {
        impl Produce for $mode {
            fn produce(common:StreamOptions<Self>,model:Model,stream:AssistantMessageEventStream) -> Self::Future<'static,()> {
                Box::pin(async move {
                    let initial_aborted = common.signal.as_ref().map(Cancellation::is_aborted);
                    let payload = common.on_payload.unwrap()(PayloadHookInput {payload:json!({"original":true}),signal:common.signal},Arc::new(model.clone())).await.unwrap();
                    common.on_response.unwrap()(ProviderResponse {status:207.5,headers:[("x-controlled".into(),"literal".into())].into()},Arc::new(model.clone())).await.unwrap();
                    let consumed = if model.id == "signal-model" {json!({"payload":payload.payload,"initialAborted":initial_aborted,"selectedAborted":payload.signal.as_ref().map(Cancellation::is_aborted)})} else {payload.payload};
                    let message = faux_assistant_message(consumed.to_string(),FauxAssistantMessageOptions::default());
                    stream.push(AssistantMessageEvent::Done {reason:DoneReason::Stop,message:Arc::new(RwLock::new(message))});
                })
            }
        }
    };
}
producer!(Local);
producer!(Threaded);
/// The actual API registry retains this controlled native adapter.
struct Hooks;
impl<E: Produce, O: Into<StreamOptionsInput<E>>> Invoke<E, ProviderCall<E, O>> for Hooks {
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, call: ProviderCall<E, O>) -> Self::Return {
        let common = match call.options.unwrap().into() {
            StreamOptionsInput::Raw(options) => options.common,
            StreamOptionsInput::Simple(options) => options.common,
        };
        let stream = AssistantMessageEventStream::new();
        E::submit(
            E::produce(common, call.model, stream.clone()),
            call.submit,
            |_| panic!("caller admission required"),
        )?;
        Ok(stream)
    }
}

#[test]
fn payload_hooks_keep_every_json_replacement() {
    super::model_hook_gates::run_cases(&[
        "keep",
        "null",
        "false",
        "zero",
        "empty-string",
        "number",
        "string",
        "object",
        "array",
    ]);
}

#[test]
fn model_hooks_preserve_replace_and_remove_preaborted_signals() {
    let provider = Arc::new(Hooks);
    register_api_provider(
        ApiProvider {
            api: "component-hooks-signals".into(),
            stream: provider.clone(),
            stream_simple: provider,
        },
        Some("component-hooks-signals".into()),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<Local>(
            std::rc::Rc::new(|_| panic!("no tails")),
            std::rc::Rc::new(|_| panic!("no errors")),
        );
        let (_owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let command = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        let output = command("model-signals".into(), runner.create_command_context())
            .await
            .unwrap_err()
            .to_string();
        let output: Value = serde_json::from_str(&output).unwrap_or_else(|_| panic!("{output}"));
        assert_eq!(output.as_array().unwrap().len(), 6);
        for row in output.as_array().unwrap() {
            let selected: Value =
                serde_json::from_str(row["message"]["content"][0]["text"].as_str().unwrap())
                    .unwrap();
            assert_eq!(selected["payload"], json!({"original":true}));
            assert_eq!(selected["initialAborted"], true);
            assert_eq!(
                selected["selectedAborted"],
                if row["action"] == "remove" {
                    Value::Null
                } else {
                    json!(true)
                }
            );
        }
        bus.clear();
    });
    unregister_api_providers("component-hooks-signals");
}
