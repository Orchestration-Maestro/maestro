//! The registered native producer waits for explicit admission, hooks and caller cancellation.
use maestro_models::*;
use serde_json::json;
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::{mpsc, oneshot};

/// Independent observations of the actual native producer and its returned buffer.
pub(super) struct Invocation {
    /// Allows hooks only after the initiating command has returned and been released.
    pub(super) start: oneshot::Sender<()>,
    /// Acknowledges both awaited hooks before the producer waits for cancellation.
    pub(super) ready: oneshot::Receiver<()>,
    /// Acknowledges actual producer completion, not just stream settlement.
    pub(super) complete: oneshot::Receiver<()>,
    /// Alias of the same native stream returned through the component boundary.
    pub(super) stream: AssistantMessageEventStream,
}
/// The real API registry retains this test provider for both execution families.
pub(super) struct Protocol(pub(super) Mutex<mpsc::UnboundedSender<Invocation>>);
/// Producer operands independent of the initiating native stack.
pub(super) struct Work {
    /// Actual supplied descriptor.
    model: Model,
    /// The buffer observed by both native and guest consumers.
    stream: AssistantMessageEventStream,
    /// Native readiness gate, not a timer.
    start: oneshot::Receiver<()>,
    /// Both callbacks have completed.
    ready: oneshot::Sender<()>,
    /// The producer has released its options and stopped.
    complete: oneshot::Sender<()>,
}
/// Preserve the existing execution-family submission and cancellation owners.
trait Produce: ProviderExecution {
    /// Invoke the actual hooks after admission, then finish only on the selected signal.
    fn produce(common: StreamOptions<Self>, work: Work) -> Self::Future<'static, ()>;
}
/// Both families execute identical provider policy without converting local hooks.
macro_rules! produce {
    ($family:ty) => {
        impl Produce for $family {
            fn produce(common: StreamOptions<Self>, work: Work) -> Self::Future<'static, ()> {
                Box::pin(async move {
                    work.start.await.unwrap();
                    let model = Arc::new(work.model);
                    let selected = common.on_payload.as_ref().unwrap()(PayloadHookInput {
                        payload: json!({"original":true}), signal: common.signal.clone(),
                    }, model.clone()).await.unwrap();
                    assert_eq!(selected.payload, json!({"selected":model.id}));
                    common.on_response.as_ref().unwrap()(ProviderResponse {
                        status:207.5, headers:[("x-controlled".into(),"literal".into())].into(),
                    }, model).await.unwrap();
                    let signal = selected.signal.unwrap();
                    assert!(!signal.is_aborted(), "another invocation cancelled this signal");
                    work.ready.send(()).unwrap();
                    signal.cancelled().await;
                    let message = faux_assistant_message("", FauxAssistantMessageOptions {
                        stop_reason:Some(StopReason::Aborted),
                        error_message:Some("Request was aborted".into()),
                        timestamp:Some(13.5), ..Default::default()
                    });
                    work.stream.push(AssistantMessageEvent::Error {
                        reason:ErrorReason::Aborted, error:Arc::new(RwLock::new(message)),
                    });
                    drop(common);
                    work.complete.send(()).unwrap();
                })
            }
        }
    };
}
produce!(Local);
produce!(Threaded);
impl<E: Produce, O: Into<StreamOptionsInput<E>>> Invoke<E, ProviderCall<E, O>> for Protocol {
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, call: ProviderCall<E, O>) -> Self::Return {
        let common = match call.options.unwrap().into() {
            StreamOptionsInput::Raw(options) => options.common,
            StreamOptionsInput::Simple(options) => options.common,
        };
        let stream = AssistantMessageEventStream::new();
        let (start, admission) = oneshot::channel();
        let (ready, entered) = oneshot::channel();
        let (complete, completed) = oneshot::channel();
        self.0
            .lock()
            .unwrap()
            .send(Invocation {
                start,
                ready: entered,
                complete: completed,
                stream: stream.clone(),
            })
            .unwrap();
        E::submit(
            E::produce(
                common,
                Work {
                    model: call.model,
                    stream: stream.clone(),
                    start: admission,
                    ready,
                    complete,
                },
            ),
            call.submit,
            |_| panic!("caller admission required"),
        )?;
        Ok(stream)
    }
}
