//! Overlapping component work and retained captures settle independently of each other.
#![cfg(test)]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions::{
    Cancellation, CommandContextOperations, ExtensionCommandContextActions,
    ExtensionContextActions, ExtensionExecution, ExtensionRunner, Local, MessageRenderOptions,
    Subscription, create_extension_runtime, types::ProviderActions,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, sync::RwLock};
use tokio::sync::{mpsc, oneshot};

use super::callback_ordering::{Instance, Pair, local};
use super::component_callbacks::{message, render, renderer};

/// Suspensions entered by command roots, with the sender that resumes each.
type Entered = mpsc::UnboundedSender<(&'static str, oneshot::Sender<()>)>;

/// The live prompt reader and signal new command contexts obtain.
fn signal_context(signal: Cancellation) -> ExtensionContextActions {
    ExtensionContextActions {
        get_system_prompt: Rc::new(|| Ok("renderer barrier".into())),
        get_signal: Rc::new(move || Ok(Some(signal.clone()))),
        ..Default::default()
    }
}

/// Binds the live prompt reader and the signal new command contexts obtain.
fn bind_signal(runner: &ExtensionRunner<Local>, signal: Cancellation) {
    runner
        .bind_core(
            Local::unbound_actions(),
            signal_context(signal),
            ProviderActions::default(),
        )
        .unwrap();
}

/// Gives `runner` its own cancellation signal and a wait the test resumes.
fn suspend(
    runner: &ExtensionRunner<Local>,
    tag: &'static str,
    entered: Entered,
    signal: Cancellation,
) {
    bind_signal(runner, signal);
    runner.bind_command_context(Some(ExtensionCommandContextActions::new(
        CommandContextOperations {
            wait_for_idle: Rc::new(move || {
                let (resume, resumed) = oneshot::channel();
                entered.send((tag, resume)).unwrap();
                Box::pin(async move {
                    resumed.await?;
                    Ok(())
                })
            }),
            ..crate::context_actions::silent_command_operations()
        },
    )));
}

/// Starts one suspended command root named `tag` on `runner`.
fn root(
    runner: &ExtensionRunner<Local>,
    tag: &str,
) -> tokio::task::JoinHandle<Result<(), maestro_extensions::EventError>> {
    let handler = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let steps = json!([{"op":"root", "tag":tag}]);
    tokio::task::spawn_local(handler(
        format!("listen:{steps}"),
        runner.create_command_context(),
    ))
}

#[test]
fn overlapping_component_roots_finish_without_stack_order() {
    local(async {
        let pair = Pair::new().await;
        pair.b
            .program(
                r#"[{"op":"on","channel":"component-foreign","id":"foreign","name":"foreign"}]"#,
            )
            .await;
        let younger_runner = ExtensionRunner::<Local>::new(
            Rc::new(RwLock::new(vec![pair.a.owner.clone()])),
            create_extension_runtime::<Local>(),
            crate::session::runner_session("/younger").0,
            Rc::new(|_| {}),
            maestro_theme::LiveTheme::default(),
        );
        let (entered, mut waiting) = mpsc::unbounded_channel();
        let (older_signal, younger_signal) = (Cancellation::new(), Cancellation::new());
        suspend(&pair.a.runner, "older", entered.clone(), older_signal);
        suspend(&younger_runner, "younger", entered, younger_signal.clone());
        let older = root(&pair.a.runner, "older");
        let (tag, resume_older) = waiting.recv().await.unwrap();
        assert_eq!(tag, "older");
        let younger = root(&younger_runner, "younger");
        let (tag, resume_younger) = waiting.recv().await.unwrap();
        assert_eq!(tag, "younger");
        assert_eq!(pair.take(), ["older:start", "younger:start"]);
        younger_signal.abort();
        resume_older.send(()).unwrap();
        older.await.unwrap().unwrap();
        assert_eq!(pair.take(), ["older:done:false"]);
        let renderer = pair
            .a
            .runner
            .get_message_renderer_call::<Value>("scoped-view")
            .unwrap();
        let message = json!({"role":"custom", "customType":"between", "content":"", "display":true, "timestamp":0});
        let view = renderer
            .call(
                Rc::new(serde_json::from_value(message).unwrap()),
                MessageRenderOptions { expanded: false },
                maestro_theme::LiveTheme::default(),
            )
            .unwrap()
            .unwrap();
        pair.trace.borrow_mut().push("caller:return".into());
        assert_eq!(
            pair.take(),
            [
                "a:create:start",
                "a:create:end",
                "foreign:prefix",
                "caller:return"
            ]
        );
        resume_younger.send(()).unwrap();
        younger.await.unwrap().unwrap();
        assert_eq!(pair.take(), ["younger:done:true"]);
        drop(view);
    });
}

#[test]
fn retained_component_subscriptions_and_tails_release_independently() {
    local(async {
        let errors = Rc::new(RefCell::new(Vec::new()));
        let reported = errors.clone();
        let mut pair = Pair::reporting(Rc::new(move |error: &str| {
            reported.borrow_mut().push(error.to_owned());
        }))
        .await;
        removal_leaves_an_admitted_tail_running(&mut pair).await;
        settled_tails_release_with_their_registration(&mut pair).await;
        faulted_instance_never_releases_through_the_guest(&mut pair).await;
        let faults = std::mem::take(&mut *errors.borrow_mut());
        assert_eq!(faults.len(), 1);
        assert!(
            faults[0].starts_with("Event handler error (fx): "),
            "{}",
            faults[0]
        );
        assert!(faults[0].contains("component fault"), "{}", faults[0]);
    });
}

/// A dropped view releases while an admitted tail and its subscription stay live.
#[test]
fn retained_view_releases_between_a_pending_tail_and_its_subscription() {
    local(async {
        let mut pair = Pair::new().await;
        let (ready, pending) = admit_waiting_tail(&mut pair).await;
        let view = render(&renderer(&pair.a, "scoped-view"), &message("scoped"), false)
            .unwrap()
            .unwrap();
        assert!(!pair.a.released("scoped"));
        drop(view);
        pair.wait_release("a:scoped").await;
        assert!(
            pair.a.released("scoped"),
            "the dropped view kept its only owner"
        );
        assert!(!pair.b.released("r"), "the view's release settled the tail");
        ready.unsubscribe();
        pair.b.program(r#"[{"op":"off","id":"r"}]"#).await;
        finish_tails(&mut pair, pending).await;
        pair.wait_release("b:r").await;
        assert!(pair.b.released("r"));
    });
}

/// Starts every queued continuation on the local task set.
fn start(pair: &mut Pair) -> Vec<tokio::task::JoinHandle<()>> {
    pair.queued()
        .into_iter()
        .map(tokio::task::spawn_local)
        .collect()
}

/// Admits `b`'s retained waiting tail, returning the subscriber and the
/// queued continuations.
async fn admit_waiting_tail(
    pair: &mut Pair,
) -> (Subscription<Local>, Vec<tokio::task::JoinHandle<()>>) {
    let (entered, waiting) = oneshot::channel();
    let entered = RefCell::new(Some(entered));
    let ready = pair.b.events.on(
        "tail-waiting",
        Rc::new(move |_, _| {
            entered.borrow_mut().take().unwrap().send(()).unwrap();
            Ok(None)
        }),
    );
    pair.b
        .program(r#"[{"op":"on","channel":"rx","id":"r","name":"r","tail":"wait","retain":true}]"#)
        .await;
    pair.bus.emit("rx", Value::Null).await;
    let pending = start(pair);
    waiting.await.unwrap();
    (ready, pending)
}

/// Unsubscribing keeps an admitted pending tail and its capture until the tail settles.
async fn removal_leaves_an_admitted_tail_running(pair: &mut Pair) {
    let signal = Cancellation::new();
    pair.b.bind_context(signal_context(signal.clone()));
    let (ready, pending) = admit_waiting_tail(pair).await;
    ready.unsubscribe();
    pair.b
        .program(r#"[{"op":"off","id":"r"},{"op":"off","id":"r"}]"#)
        .await;
    pair.trace.borrow_mut().push("unsubscribe".into());
    pair.b.barrier().await;
    assert!(!pair.b.released("r"));
    let rebound = Cancellation::new();
    pair.b.bind_context(signal_context(rebound.clone()));
    rebound.abort();
    assert!(!signal.is_aborted());
    finish_tails(pair, pending).await;
    assert_eq!(
        pair.take(),
        [
            "r:prefix",
            "unsubscribe",
            "open:prefix",
            "open:tail",
            "r:tail:false"
        ]
    );
    pair.wait_release("b:r").await;
    assert!(pair.b.released("r"));
    pair.bus.emit("rx", Value::Null).await;
    assert!(pair.take().is_empty() && pair.queued().is_empty());
}

/// Opens the authored gate and waits for both tails to settle.
async fn finish_tails(pair: &mut Pair, pending: Vec<tokio::task::JoinHandle<()>>) {
    pair.b
        .program(r#"[{"op":"on","channel":"open","id":"o","name":"open","tail":"open"}]"#)
        .await;
    pair.bus.emit("open", Value::Null).await;
    let opener = start(pair);
    for task in pending.into_iter().chain(opener) {
        task.await.unwrap();
    }
}

/// A settled tail leaves the capture with its registration; dropped handles keep listening.
async fn settled_tails_release_with_their_registration(pair: &mut Pair) {
    let steps = json!([
        {"op":"on", "channel":"ig", "id":"first", "name":"first"},
        {"op":"on", "channel":"ig", "id":"second", "name":"second"},
        {"op":"forget", "id":"first"},
        {"op":"forget", "id":"second"},
        {"op":"on", "channel":"cx", "id":"c", "name":"c", "tail":"ready", "retain":true},
    ]);
    pair.a.program(&steps.to_string()).await;
    pair.bus.emit("ig", Value::Null).await;
    pair.bus.emit("cx", Value::Null).await;
    for task in start(pair) {
        task.await.unwrap();
    }
    assert_eq!(
        pair.take(),
        ["first:prefix", "second:prefix", "c:prefix", "c:tail"]
    );
    pair.a.barrier().await;
    assert!(!pair.a.released("c"));
    pair.bus.clear();
    pair.observe();
    pair.wait_release("a:c").await;
    assert!(pair.a.released("c"));
    pair.a
        .program(r#"[{"op":"on","channel":"ig","id":"after","name":"after-clear"}]"#)
        .await;
    pair.bus.emit("ig", Value::Null).await;
    assert_eq!(pair.take(), ["after-clear:prefix"]);
}

/// A fault settles the admitted tail and leaves its capture unreleased by guest code.
async fn faulted_instance_never_releases_through_the_guest(pair: &mut Pair) {
    pair.a
        .program(r#"[{"op":"on","channel":"fx","id":"f","name":"f","tail":"wait","retain":true}]"#)
        .await;
    pair.bus.emit("fx", Value::Null).await;
    let pending = start(pair);
    trap(&pair.a).await;
    for task in pending {
        task.await.unwrap();
    }
    pair.bus.clear();
    pair.observe();
    assert!(!pair.a.released("f"));
    assert_eq!(pair.take(), ["f:prefix"]);
}

/// Traps the instance through its existing command.
async fn trap(instance: &Instance) {
    instance
        .runner
        .bind_command_context(Some(ExtensionCommandContextActions::new(
            crate::context_actions::silent_command_operations(),
        )));
    let handler = instance
        .runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let fault = handler("trap".into(), instance.runner.create_command_context())
        .await
        .unwrap_err();
    assert!(fault.to_string().contains("component fault"), "{fault}");
}
