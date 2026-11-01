//! Native protocol fixtures observe original response bodies without reproducing provider policy.
use maestro_models::*;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::Poll,
};

/// Native body effects, without retaining the body itself or recording cumulative events.
#[derive(Default)]
pub(super) struct Body {
    /// Every actual native body poll.
    polls: AtomicUsize,
    /// Final release of that body, including rejected response attempts.
    drops: AtomicUsize,
}
impl Body {
    /// Read native consumption at the actual body boundary.
    pub(super) fn polls(&self) -> usize {
        self.polls.load(Ordering::SeqCst)
    }
    /// Read native ownership release independently of callback completion.
    pub(super) fn drops(&self) -> usize {
        self.drops.load(Ordering::SeqCst)
    }
}
/// One observed native transport per protocol; no guest callback or provider state lives here.
#[derive(Default)]
pub(super) struct Probe {
    /// Counters for each native attempt in dispatch order.
    pub(super) bodies: Mutex<Vec<Arc<Body>>>,
}
/// Exact bundled protocol selected by the ordinary native registry.
pub(super) struct Protocol {
    /// Original adapter API, never inferred from a hook's result.
    pub(super) api: &'static str,
    /// Transport observations only.
    pub(super) probe: Arc<Probe>,
}
impl<E: ProviderExecution, O: Into<StreamOptionsInput<E>>> Invoke<E, ProviderCall<E, O>>
    for Protocol
{
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, mut call: ProviderCall<E, O>) -> Self::Return {
        call.model.api = self.api.into();
        let original = get_api_provider(self.api).unwrap();
        let fetch = self.fetch();
        match call.options.unwrap().into() {
            StreamOptionsInput::Raw(mut options) => {
                options.common.fetch = Some(fetch);
                original.stream.call(
                    call.model,
                    call.context,
                    E::submission(Some(options), call.submit),
                )
            }
            StreamOptionsInput::Simple(mut options) => {
                options.common.fetch = Some(fetch);
                original.stream_simple.call(
                    call.model,
                    call.context,
                    E::submission(Some(options), call.submit),
                )
            }
        }
    }
}
impl Protocol {
    /// Native adapters consume finite protocol bytes and retain the original guest hook handles.
    fn fetch(&self) -> Fetch {
        let (probe, api) = (self.probe.clone(), self.api);
        Arc::new(move |request| {
            assert_eq!(request.method, "POST");
            assert!(!request.body.is_empty());
            let body = Arc::new(Body::default());
            probe.bodies.lock().unwrap().push(body.clone());
            let release = Released(body.clone());
            let mut bytes = Some(response(api).as_bytes().to_vec());
            let body: HttpBody = Box::pin(futures_util::stream::poll_fn(move |_| {
                let _ = &release;
                body.polls.fetch_add(1, Ordering::SeqCst);
                Poll::Ready(bytes.take().map(Ok))
            }));
            Box::pin(std::future::ready(Ok(HttpResponse {
                status: 200,
                status_text: "OK".into(),
                headers: [
                    ("content-type".into(), "text/event-stream".into()),
                    ("x-controlled".into(), "literal".into()),
                ]
                .into(),
                body,
            })))
        })
    }
}
/// The actual native body capture reports exactly one final release, even if never polled.
struct Released(Arc<Body>);
impl Drop for Released {
    fn drop(&mut self) {
        self.0.drops.fetch_add(1, Ordering::SeqCst);
    }
}
/// Literal protocol answers consumed by the bundled reducers; no reducer is duplicated here.
fn response(api: &str) -> &'static str {
    match api {
        "google-generative-ai" | "google-vertex" => {
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"routed\"}],\"role\":\"model\"},\"finishReason\":\"STOP\"}]}\n\n"
        }
        "openai-codex-responses" => concat!(
            "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"message\",\"id\":\"m\",\"content\":[]}}\n\n",
            "data: {\"type\":\"response.content_part.added\",\"part\":{\"type\":\"output_text\",\"text\":\"\"}}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"routed\"}\n\n",
            "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"message\",\"id\":\"m\",\"content\":[{\"type\":\"output_text\",\"text\":\"routed\"}]}}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r\",\"status\":\"completed\",\"usage\":{\"input_tokens\":3,\"output_tokens\":2,\"total_tokens\":5}}}\n\n"
        ),
        _ => concat!(
            "data: {\"id\":\"local\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"routed\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"local\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n"
        ),
    }
}
