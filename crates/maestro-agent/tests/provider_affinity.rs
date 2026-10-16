//! Family-correct model hooks through owned agent execution.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_agent::{Agent, AgentOptions, Local, NoCustomMessages, PromptInput};
use maestro_models::{AssistantContent, FauxResponseStep, RegisterFauxProviderOptions};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

#[test]
fn local_agent_constructor_hooks_reach_registered_dispatch() {
    let registration = maestro_models::register_faux_provider(RegisterFauxProviderOptions {
        api: Some("local-agent-constructor".into()),
        ..Default::default()
    });
    registration.set_responses(vec![FauxResponseStep::Factory(Arc::new(SelectedPayload))]);
    let tasks = Rc::new(RefCell::new(Vec::new()));
    let queued = Rc::clone(&tasks);
    let observations = Rc::new(Cell::new(0));
    let observed = Rc::clone(&observations);
    let agent = Rc::new(Agent::<NoCustomMessages, Local>::new_local(AgentOptions {
        spawn: Some(Rc::new(move |task| queued.borrow_mut().push(task))),
        on_payload: Some(payload_hook("11")),
        on_response: Some(Rc::new(move |_, _| {
            let observed = Rc::clone(&observed);
            Box::pin(async move {
                tokio::task::yield_now().await;
                observed.set(observed.get() + 1);
                Ok(())
            })
        })),
        ..Default::default()
    }));
    *agent.state().write().unwrap().model.write().unwrap() =
        registration.get_model(None).unwrap().clone();
    let observe = agent.prompt(PromptInput::Text {
        text: "prompt".into(),
        images: vec![],
    });
    let owned = tasks.borrow_mut().pop().unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(tokio::task::LocalSet::new().run_until(async {
        let running = tokio::task::spawn_local(owned);
        tokio::task::yield_now().await;
        let provider = tasks.borrow_mut().pop().unwrap();
        provider.await;
        running.await.unwrap();
        observe.await.unwrap();
    }));
    assert_eq!(observations.get(), 1);
    let state = agent.state().read().unwrap();
    let history = state.messages().read().unwrap();
    let maestro_agent::AgentMessage::Assistant(message) = history.last().unwrap() else {
        panic!("missing assistant")
    };
    assert!(
        matches!(&message.read().unwrap().content[0], AssistantContent::Text(text) if text.text == "11")
    );
    registration.unregister();
}

/// Factory consuming the payload selected by the original invocation hook.
struct SelectedPayload;
impl SelectedPayload {
    /// Execute against the original family's options after response observation.
    async fn produce<E: maestro_models::ProviderExecution>(
        input: maestro_models::FauxResponseInput<E>,
    ) -> Result<maestro_models::AssistantMessage, maestro_models::DiagnosticErrorInfo> {
        let common = match input.options.unwrap() {
            maestro_models::StreamOptionsInput::Raw(options) => options.common,
            maestro_models::StreamOptionsInput::Simple(options) => options.common,
        };
        let selected = common.on_payload.unwrap()(
            maestro_models::PayloadHookInput {
                payload: serde_json::json!({"text": "unselected"}),
                signal: common.signal,
            },
            Arc::new(input.model),
        )
        .await?;
        Ok(
            support::assistant(selected.payload["text"].as_str().unwrap())
                .read()
                .unwrap()
                .clone(),
        )
    }
}
/// Concrete future erasure around the single generic factory body.
macro_rules! factory {
    ($mode:ty) => {
        impl maestro_models::Invoke<$mode, maestro_models::FauxResponseInput<$mode>>
            for SelectedPayload
        {
            type Return = <$mode as maestro_models::Execution>::Future<
                'static,
                Result<maestro_models::AssistantMessage, maestro_models::DiagnosticErrorInfo>,
            >;
            fn invoke(&self, input: maestro_models::FauxResponseInput<$mode>) -> Self::Return {
                Box::pin(Self::produce(input))
            }
        }
    };
}
factory!(Local);
factory!(maestro_models::Threaded);

/// A local payload selection that retains its capture across suspension.
fn payload_hook(text: &'static str) -> maestro_models::OnPayload<Local> {
    let selected = Rc::new(text);
    Rc::new(move |mut input, _| {
        let selected = Rc::clone(&selected);
        Box::pin(async move {
            tokio::task::yield_now().await;
            input.payload["text"] = serde_json::json!(*selected);
            Ok(input)
        })
    })
}
