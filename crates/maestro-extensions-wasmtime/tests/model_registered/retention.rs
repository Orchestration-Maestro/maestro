//! Returned guest streams retain original native controls beyond their containing registration.
use super::*;
use std::cell::RefCell;

/// Healthy and externally cancelled producers outlive every initiating registration owner.
pub(super) fn run() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        for simple in [false, true] {
            for action in ["keep", "abort"] {
                retained(simple, action).await;
            }
        }
    });
}

/// Release the actual containing owners while the original native hooks are still parked.
async fn retained(simple: bool, action: &str) {
    let errors = Rc::new(RefCell::new(Vec::new()));
    let (reporter, mut teardown) = super::super::lifetimes::observed_reporter(&errors);
    let bus = maestro_extensions::create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        reporter,
    );
    let (owner, runner, events) =
        super::super::retained_components::loaded_on(bus.for_extension()).await;
    register(&runner, "create").await;
    let (entered, mut incoming) = mpsc::unbounded_channel();
    let signal = Cancellation::new();
    let (common, mut released) = guarded(options("keep", entered, signal.clone()), action);
    let stream = start(simple, common);
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let producer = command(
        "model-registered produce".into(),
        runner.create_command_context(),
    );
    drop(command);
    let producer = tokio::task::spawn_local(producer);
    let (phase, resume) = incoming.recv().await.unwrap();
    assert_eq!(phase, "payload");
    unregister_api_providers("provider:compiled-stream");
    bus.clear();
    drop((owner, runner, events, bus));
    unretired(&mut teardown);
    unretired(&mut released);
    pending(&stream);
    resume.send(()).unwrap();
    if action == "abort" {
        signal.abort();
    } else {
        let (phase, resume) = incoming.recv().await.unwrap();
        assert_eq!(phase, "response");
        unretired(&mut teardown);
        unretired(&mut released);
        pending(&stream);
        resume.send(()).unwrap();
    }
    assert_eq!(producer.await.unwrap().unwrap_err().to_string(), "produced");
    check(&stream, action).await;
    unretired(&mut teardown);
    drop(stream);
    released.await.unwrap();
    teardown.await.unwrap();
    assert!(errors.borrow().is_empty());
}

/// Native capture destruction reenters stream creation and reports its final release once.
struct Released(Option<oneshot::Sender<()>>);
impl Drop for Released {
    fn drop(&mut self) {
        let stream = AssistantMessageEventStream::new();
        stream.end(None);
        let _ = self.0.take().unwrap().send(());
    }
}

/// Both supplied native hooks share one non-serializable release owner.
fn guarded(
    mut common: StreamOptions<Local>,
    action: &str,
) -> (StreamOptions<Local>, oneshot::Receiver<()>) {
    let (sender, released) = oneshot::channel();
    let owner = Rc::new(Released(Some(sender)));
    let payload = common.on_payload.take().unwrap();
    let payload_owner = owner.clone();
    let action = action.to_owned();
    common.on_payload = Some(Rc::new(move |input, model| {
        let (result, owner, action) =
            (payload(input, model), payload_owner.clone(), action.clone());
        Box::pin(async move {
            let _retained = owner;
            let selected = result.await?;
            if action == "abort" {
                selected.signal.as_ref().unwrap().cancelled().await;
            }
            Ok(selected)
        })
    }));
    let response = common.on_response.take().unwrap();
    common.on_response = Some(Rc::new(move |input, model| {
        let (result, owner) = (response(input, model), owner.clone());
        Box::pin(async move {
            let _retained = owner;
            result.await
        })
    }));
    (common, released)
}

/// A final lease, not a stale registration or a timer, determines containing-owner lifetime.
fn unretired(receiver: &mut oneshot::Receiver<()>) {
    assert!(matches!(
        receiver.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
}
