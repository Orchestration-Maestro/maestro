//! Tool registration invokes actual native preparation and execution callbacks.
use crate::lifecycle::replies::Replies;
use crate::read_imports::ReadEndpoint;
use crate::{
    context_reads::Request as Read,
    guest_calls::{Call, Callbacks, ToolCall},
    local_resources::Pump,
    registration_reads::Request,
};
use maestro_extensions::{Cancellation, Local, ToolExecutionMode};
use serde_json::json;
use std::{cell::RefCell, rc::Rc};

#[test]
fn registered_tool_preserves_metadata_preparation_and_execution_capabilities() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(tool());
}

/// Drives the real native definition and its fresh per-invocation context.
async fn tool() {
    let (owner, api) = super::api::api(maestro_extensions::create_extension_runtime::<Local>());
    let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
    let endpoint = pump.registry().api(api).unwrap();
    let (calls, incoming) = tokio::sync::mpsc::unbounded_channel();
    let mut callbacks = Callbacks::new(calls.downgrade(), Replies::default());
    let prepare = callbacks.allocate().unwrap();
    let prepare = callbacks.retain(prepare.id, admission.clone()).unwrap();
    let execute = callbacks.allocate().unwrap();
    let execute = callbacks.retain(execute.id, admission.clone()).unwrap();
    let worker = std::thread::spawn(move || {
        endpoint
            .registration(|reply| Request::Tool {
                schemas: vec![],
                renderers: (None, None),
                metadata: json!({"name":" exact ","label":" Label ","description":" desc ",
                "parameters":{"type":"object"},"promptSnippet":"",
                "promptGuidelines":[],"executionMode":"parallel","renderShell":"self"})
                .to_string(),
                prepare: Some(prepare),
                execute,
                reply,
            })
            .unwrap();
    });
    assert!(pump.answer_next().await);
    worker.join().unwrap();
    let definition = owner.read().unwrap().tools[" exact "].clone();
    metadata(&definition);
    let runner = Rc::new(super::runner("/native-tool"));
    let wrapped = maestro_extensions::wrap_registered_tool(&definition, runner);
    let prepare = wrapped.prepare_arguments.clone().unwrap();
    routed_preparation(&mut pump, move || prepare(&mut json!({"nested":true}))).await;
    let worker = std::thread::spawn(move || invoke(incoming));
    let mut args = json!({"before":true});
    args = wrapped.prepare_arguments.as_ref().unwrap()(&mut args).unwrap();
    assert_eq!(args, json!({"prepared":true}));
    let updates = Rc::new(RefCell::new(Vec::new()));
    let observed = updates.clone();
    let signal = Cancellation::new();
    signal.abort();
    let work = (wrapped.execute)(
        " literal id ".into(),
        args,
        Some(signal),
        Some(Rc::new(move |partial| {
            observed.borrow_mut().push(partial);
        })),
    );
    let result = drive(&mut pump, work).await.unwrap();
    assert_eq!(result.details, Some(json!({"done":true})));
    assert_eq!(result.terminate, Some(false));
    assert_eq!(updates.borrow().len(), 1);
    worker.join().unwrap();
    drop((owner, wrapped, definition, admission, calls));
    pump.serve().await;
}

/// Answers a preparation made while the Store waits in a routed request, through that route.
async fn routed_preparation<E: std::fmt::Debug + 'static>(
    pump: &mut Pump,
    prepare: impl FnOnce() -> Result<serde_json::Value, E> + 'static,
) {
    let nested = super::during_routed_request(pump, prepare, |call| {
        let Call::Tool(ToolCall::Prepare { args, reply, .. }) = call else {
            panic!("missing nested preparation")
        };
        assert_eq!(args, r#"{"nested":true}"#);
        reply
            .send(Ok(crate::guest_exports::tools::PreparedArguments {
                retained: args,
                prepared: Ok(r#"{"nested":"prepared"}"#.into()),
            }))
            .unwrap();
    })
    .await;
    assert_eq!(nested.unwrap(), json!({"nested":"prepared"}));
}

/// Observes typed Store messages and exercises the original capability endpoints.
fn invoke(mut incoming: tokio::sync::mpsc::UnboundedReceiver<Call>) {
    let Call::Tool(ToolCall::Prepare {
        args,
        emitter,
        reply,
        ..
    }) = incoming.blocking_recv().unwrap()
    else {
        panic!("missing preparation")
    };
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&args).unwrap(),
        json!({"before":true})
    );
    drop(emitter);
    reply
        .send(Ok(crate::guest_exports::tools::PreparedArguments {
            retained: args,
            prepared: Ok("{\"prepared\":true}".into()),
        }))
        .unwrap();
    let call = loop {
        match incoming.blocking_recv().unwrap() {
            Call::Release(_) => {}
            call => break call,
        }
    };
    let Call::Tool(ToolCall::Execute {
        id,
        params,
        context,
        signal,
        update,
        reply,
        ..
    }) = call
    else {
        panic!("missing execution")
    };
    assert_eq!(id, " literal id ");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&params).unwrap(),
        json!({"prepared":true})
    );
    assert_eq!(context.read(Read::Cwd).unwrap(), "/native-tool");
    assert!(signal.unwrap().aborted().unwrap());
    update
        .unwrap()
        .update(r#"{"content":[],"details":{"part":1}}"#.into())
        .result()
        .unwrap();
    reply
        .send(Ok(
            r#"{"content":[],"details":{"done":true},"terminate":false}"#.into(),
        ))
        .unwrap();
}

/// Services the sole request pump until the native invocation settles.
async fn drive<F: std::future::Future>(pump: &mut Pump, work: F) -> F::Output {
    match futures_util::future::select(
        Box::pin(work),
        Box::pin(async { while pump.answer_next().await {} }),
    )
    .await
    {
        futures_util::future::Either::Left((result, _)) => result,
        futures_util::future::Either::Right(_) => panic!("pump ended before tool"),
    }
}

/// Checks the real native metadata rather than a parallel copy of the document.
fn metadata(definition: &maestro_extensions::RegisteredTool<Local>) {
    let metadata = definition.definition.metadata();
    assert_eq!(metadata.definition.description, " desc ");
    assert_eq!(
        metadata.definition.parameters.as_json(),
        &json!({"type":"object"})
    );
    assert_eq!(metadata.prompt_snippet.as_deref(), Some(""));
    assert_eq!(metadata.prompt_guidelines, Some(Vec::new()));
    assert_eq!(metadata.execution_mode, Some(ToolExecutionMode::Parallel));
    assert_eq!(
        metadata.render_shell,
        Some(maestro_extensions::ToolRenderShell::SelfRendered)
    );
}

#[test]
fn malformed_tool_documents_never_register_a_native_definition() {
    tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async {
        let (owner, api) = super::api::api(maestro_extensions::create_extension_runtime::<Local>());
        let (pump, admission) = Pump::new(crate::tests::support::widgets());
        let endpoint = pump.registry().api(api).unwrap();
        let (calls, _incoming) = tokio::sync::mpsc::unbounded_channel();
        let mut callbacks = Callbacks::new(calls.downgrade(), Replies::default());
        let handler = callbacks.allocate().unwrap();
        let handler = callbacks.retain(handler.id, admission.clone()).unwrap();
        let worker = std::thread::spawn(move || {
            for metadata in [
                "not JSON".to_owned(),
                json!({"name":"bad","label":"bad","description":"bad"}).to_string(),
                json!({"name":"bad","label":"bad","description":"bad","parameters":{},"renderShell":"unknown"}).to_string(),
                json!({"name":"bad","label":"bad","description":"bad","parameters":{},"executionMode":"unknown"}).to_string(),
            ] {
                assert!(endpoint.registration(|reply| Request::Tool { schemas: vec![], renderers: (None, None),
                    metadata, prepare: None, execute: handler.clone(), reply,
                }).is_err());
            }
        });
        drop(admission);
        pump.serve().await;
        worker.join().unwrap();
        assert!(owner.read().unwrap().tools.is_empty());
        drop(calls);
    });
}
