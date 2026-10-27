//! Real registration records select the native caller's context and guest operands.
use crate::lifecycle::replies::Replies;
use crate::read_imports::ReadEndpoint;
use crate::{
    context_reads::Request as Read,
    guest_calls::{Call, Callbacks, Handle, RegisteredCall},
    local_resources::{Admission, Pump},
    registration_reads::Request,
};
use maestro_extensions::{
    Local,
    types::{ArgumentCompletions, CommandHandler, ShortcutHandler},
};

#[test]
fn registered_command_completion_and_shortcut_invoke_with_native_operands() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(registrations());
}

/// Runs native callbacks while the original pump consumes their context reads.
async fn registrations() {
    let (owner, api) = super::api::api(maestro_extensions::create_extension_runtime::<Local>());
    let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
    let api = pump.registry().api(api).unwrap();
    let (calls, incoming) = tokio::sync::mpsc::unbounded_channel();
    let mut callbacks = Callbacks::new(calls.downgrade(), Replies::default());
    let handler = retained(&mut callbacks, &admission);
    let completion = retained(&mut callbacks, &admission);
    let shortcut = retained(&mut callbacks, &admission);
    let worker = std::thread::spawn(move || register(&api, handler, completion, shortcut));
    for _ in 0..2 {
        assert!(pump.answer_next().await);
    }
    worker.join().unwrap();
    let (command, completion, shortcut) = registered(&owner);
    let worker = std::thread::spawn(move || invoke(incoming));
    let runner = crate::tests::runner("/native-command");
    let result = drive(
        &mut pump,
        command(" raw args ".into(), runner.create_command_context()),
    )
    .await;
    assert_eq!(result.unwrap_err().to_string(), "command:/native-command");
    let items = completion(" raw prefix ".into()).await.unwrap().unwrap();
    assert_eq!(items[0].value, " value ");
    assert_eq!(items[0].label, " label ");
    assert_eq!(items[0].description.as_deref(), Some(""));
    let runner = crate::tests::runner("/native-shortcut");
    let result = drive(&mut pump, shortcut(runner.create_context())).await;
    assert_eq!(result.unwrap_err().to_string(), "shortcut:/native-shortcut");
    worker.join().unwrap();
    drop((owner, command, completion, shortcut, admission, calls));
    pump.serve().await;
}

/// Allocates distinct authored callbacks sharing the original pump admission.
fn retained(callbacks: &mut Callbacks, admission: &Admission) -> Handle {
    let callback = callbacks.allocate().unwrap();
    callbacks.retain(callback.id, admission.clone()).unwrap()
}

/// Registers through the typed Store endpoint, not through a second registration map.
fn register(
    api: &crate::api_reads::Endpoint,
    handler: Handle,
    completion: Handle,
    shortcut: Handle,
) {
    api.registration(|reply| Request::Command {
        name: " exact ".into(),
        description: Some(String::new()),
        completions: Some(completion),
        handler,
        reply,
    })
    .unwrap();
    api.registration(|reply| Request::Shortcut {
        key: "ctrl+alt+x".into(),
        description: None,
        handler: shortcut,
        reply,
    })
    .unwrap();
}

/// Reads real native records, retaining their actual callbacks and provenance.
fn registered(
    owner: &maestro_extensions::types::ExtensionHandle<Local>,
) -> (
    CommandHandler<Local>,
    ArgumentCompletions<Local>,
    ShortcutHandler<Local>,
) {
    let owner = owner.read().unwrap();
    let command = owner.commands[" exact "].read().unwrap();
    assert_eq!(command.name, " exact ");
    assert_eq!(command.description.as_deref(), Some(""));
    assert!(std::sync::Arc::ptr_eq(
        &command.source_info,
        &owner.source_info
    ));
    (
        command.handler.clone(),
        command.get_argument_completions.clone().unwrap(),
        owner.shortcuts["ctrl+alt+x"]
            .read()
            .unwrap()
            .handler
            .clone(),
    )
}

/// Observes the original native operands at the Store-safe guest-call boundary.
fn invoke(mut incoming: tokio::sync::mpsc::UnboundedReceiver<Call>) {
    let Call::Registered(RegisteredCall::Command(_, args, context, reply)) =
        incoming.blocking_recv().unwrap()
    else {
        panic!("missing command")
    };
    assert_eq!(args, " raw args ");
    let cwd = context.read(Read::Cwd).unwrap();
    reply.send(Err(format!("command:{cwd}"))).unwrap();
    let Call::Registered(RegisteredCall::Completion(_, prefix, reply)) =
        incoming.blocking_recv().unwrap()
    else {
        panic!("missing completion")
    };
    assert_eq!(prefix, " raw prefix ");
    reply
        .send(Ok(Some(vec![maestro_extensions::AutocompleteItem {
            value: " value ".into(),
            label: " label ".into(),
            description: Some(String::new()),
        }])))
        .unwrap();
    let Call::Registered(RegisteredCall::Shortcut(_, context, reply)) =
        incoming.blocking_recv().unwrap()
    else {
        panic!("missing shortcut")
    };
    let cwd = context.read(Read::Cwd).unwrap();
    reply.send(Err(format!("shortcut:{cwd}"))).unwrap();
}

/// Keeps consuming release and read messages until the real native callback settles.
async fn drive<F: std::future::Future>(pump: &mut Pump, work: F) -> F::Output {
    let service = Box::pin(async { while pump.answer_next().await {} });
    match futures_util::future::select(Box::pin(work), service).await {
        futures_util::future::Either::Left((result, _)) => result,
        futures_util::future::Either::Right(_) => panic!("pump ended before callback"),
    }
}
