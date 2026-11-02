//! Observe the real HTTP client and its original streaming body without replacing protocol policy.
use futures_util::stream::poll_fn;
use maestro_models::*;
use serde_json::Value;
use std::{
    future::Future,
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Waker},
};

/// Phase effects and a native alias of the same stream returned to the guest.
#[derive(Default)]
pub(super) struct Probe {
    /// Explicit acknowledgment of the guest payload and response gates.
    phase: AtomicUsize,
    /// Calls at the actual HTTP dispatch boundary.
    dispatches: AtomicUsize,
    /// Polls of the actual response body, including a pending first poll.
    body_polls: AtomicUsize,
    /// Release of the actual HTTP body after consumption or callback failure.
    body_drops: AtomicUsize,
    /// Same native FIFO/result owner, not an event recorder.
    stream: Mutex<Option<AssistantMessageEventStream>>,
}
impl Probe {
    /// Observe dispatch before acknowledging its awaited hook.
    pub(super) fn dispatches(&self) -> usize {
        self.dispatches.load(Ordering::SeqCst)
    }
    /// Observe body consumption before acknowledging its awaited hook.
    pub(super) fn body_polls(&self) -> usize {
        self.body_polls.load(Ordering::SeqCst)
    }
    /// Observe that producer teardown released the actual response body exactly once.
    pub(super) fn body_drops(&self) -> usize {
        self.body_drops.load(Ordering::SeqCst)
    }
    /// Acknowledge payload readiness on the same invocation.
    pub(super) fn allow_payload(&self) {
        self.phase.store(1, Ordering::SeqCst);
    }
    /// Acknowledge response readiness on the same invocation.
    pub(super) fn allow_response(&self) {
        self.phase.store(2, Ordering::SeqCst);
    }
    /// Both the native event and result observers must still be pending at either hook gate.
    pub(super) fn assert_pending(&self) {
        let stream = self.stream.lock().unwrap().as_ref().unwrap().clone();
        let mut context = Context::from_waker(Waker::noop());
        let mut next = std::pin::pin!(stream.next());
        assert!(next.as_mut().poll(&mut context).is_pending());
        let mut result = std::pin::pin!(stream.result());
        assert!(result.as_mut().poll(&mut context).is_pending());
    }
    /// Count the original body's first poll even when it has no chunk ready.
    fn observe_body(self: Arc<Self>, mut body: HttpBody) -> HttpBody {
        let release = BodyRelease(self.clone());
        Box::pin(poll_fn(move |context| {
            let _ = &release;
            assert_eq!(
                self.phase.load(Ordering::SeqCst),
                2,
                "body consumed before the awaited response hook returned"
            );
            self.body_polls.fetch_add(1, Ordering::SeqCst);
            body.as_mut().poll_next(context)
        }))
    }
    /// Wrap only observations around the production HTTP client and original response body.
    fn fetch(self: &Arc<Self>) -> Fetch {
        let probe = self.clone();
        Arc::new(move |request| {
            assert_eq!(probe.phase.load(Ordering::SeqCst), 1);
            probe.dispatches.fetch_add(1, Ordering::SeqCst);
            let probe = probe.clone();
            Box::pin(async move {
                let mut response = default_fetch()(request).await?;
                response.body = probe.observe_body(response.body);
                Ok(response)
            })
        })
    }
}

/// The original bundled protocol consumes the guest options using caller-owned submission.
pub(super) struct Protocol(pub(super) Arc<Probe>);
impl<E: ProviderExecution, O: Into<StreamOptionsInput<E>>> Invoke<E, ProviderCall<E, O>>
    for Protocol
{
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, mut call: ProviderCall<E, O>) -> Self::Return {
        call.model.api = "openai-completions".into();
        let original = get_api_provider("openai-completions").unwrap();
        let stream = match call.options.unwrap().into() {
            StreamOptionsInput::Raw(mut options) => {
                options.common.fetch = Some(self.0.fetch());
                original.stream.call(
                    call.model,
                    call.context,
                    E::submission(Some(options), call.submit),
                )?
            }
            StreamOptionsInput::Simple(mut options) => {
                options.common.fetch = Some(self.0.fetch());
                original.stream_simple.call(
                    call.model,
                    call.context,
                    E::submission(Some(options), call.submit),
                )?
            }
        };
        *self.0.stream.lock().unwrap() = Some(stream.clone());
        Ok(stream)
    }
}

/// A local protocol peer sends headers and a finite body without waiting for the guest gate.
pub(super) struct Server {
    /// Actual request origin supplied to the compiled guest.
    pub(super) url: String,
    /// Joins the real peer and retrieves its independently observed request body.
    thread: std::thread::JoinHandle<Value>,
}
impl Server {
    /// Accept one finite streaming protocol request without any live provider.
    pub(super) fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        let thread = std::thread::spawn(move || {
            let (mut peer, _) = listener.accept().unwrap();
            let request = read_request(&mut peer);
            let body = concat!(
                "data: {\"id\":\"local\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"native body 雪\"},\"finish_reason\":null}]}\n\n",
                "data: {\"id\":\"local\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
                "data: [DONE]\n\n"
            );
            write!(peer, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nX-Controlled: literal\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            peer.flush().unwrap();
            request
        });
        Self { url, thread }
    }
    /// The normal terminal result proves the finite peer has finished its response.
    pub(super) fn finish(self) -> Value {
        self.thread.join().unwrap()
    }
}

/// Read the client's content-length framed request at the actual network consumer.
fn read_request(peer: &mut std::net::TcpStream) -> Value {
    let mut bytes = Vec::new();
    let mut byte = [0];
    while !bytes.ends_with(b"\r\n\r\n") {
        peer.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
    }
    let headers = String::from_utf8(bytes).unwrap();
    assert!(headers.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"));
    let length: usize = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().unwrap())
        })
        .unwrap();
    let mut body = vec![0; length];
    peer.read_exact(&mut body).unwrap();
    serde_json::from_slice(&body).unwrap()
}

/// A capture belonging only to the actual native response-body stream.
struct BodyRelease(Arc<Probe>);
impl Drop for BodyRelease {
    fn drop(&mut self) {
        self.0.body_drops.fetch_add(1, Ordering::SeqCst);
    }
}
