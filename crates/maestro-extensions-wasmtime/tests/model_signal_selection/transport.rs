//! Observe the bundled content adapter's actual HTTP future without replacing its cancellation policy.
use maestro_models::*;
use serde_json::{Value, json};
use std::{
    future::{Future, poll_fn},
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Waker},
};
use tokio::sync::{mpsc, oneshot};

/// Poll acknowledgments and final release of the original HTTP operation.
pub(super) enum Observation {
    /// The real pending future acknowledged this requested generation.
    Polled(usize),
    /// The real HTTP future was released.
    Dropped,
}

/// Only observations; the provider and default HTTP client retain all execution policy.
pub(super) struct Probe {
    /// Explicitly requested transport-poll generation.
    generation: AtomicUsize,
    /// Last acknowledged generation.
    acknowledged: AtomicUsize,
    /// Original operation's task waker.
    waker: Mutex<Option<Waker>>,
    /// Transport-poll and drop acknowledgments.
    observations: mpsc::UnboundedSender<Observation>,
    /// Original option cancellation, before payload selection.
    original: Mutex<Option<Cancellation>>,
    /// The native content owner derives this transport flag only for a selected signal.
    transport: Mutex<Option<Cancellation>>,
    /// Same returned native result owner, not a second event recorder.
    stream: Mutex<Option<AssistantMessageEventStream>>,
}
impl Probe {
    /// Every case has its own acknowledgment channel.
    pub(super) fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<Observation>) {
        let (observations, incoming) = mpsc::unbounded_channel();
        (
            Arc::new(Self {
                generation: AtomicUsize::new(0),
                acknowledged: AtomicUsize::new(usize::MAX),
                waker: Mutex::default(),
                observations,
                original: Mutex::default(),
                transport: Mutex::default(),
                stream: Mutex::default(),
            }),
            incoming,
        )
    }
    /// A result remains unobservable while the selected transport is genuinely pending.
    fn assert_pending(&self) {
        let stream = self.stream.lock().unwrap().as_ref().unwrap().clone();
        let mut result = std::pin::pin!(stream.result());
        assert!(
            result
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
    }
    /// Require another poll of the real pending HTTP operation, not a scheduler delay.
    async fn checkpoint(&self, incoming: &mut mpsc::UnboundedReceiver<Observation>) {
        self.assert_pending();
        let wanted = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.waker.lock().unwrap().as_ref().unwrap().wake_by_ref();
        loop {
            match incoming.recv().await.unwrap() {
                Observation::Polled(generation) if generation == wanted => break,
                Observation::Polled(_) => {}
                Observation::Dropped => panic!("unselected signal cancelled the pending request"),
            }
        }
        self.assert_pending();
    }
    /// Count actual pending polls and retain the real HTTP client's own waker.
    fn fetch(self: &Arc<Self>) -> Fetch {
        let probe = self.clone();
        Arc::new(move |request| {
            *probe.transport.lock().unwrap() = request.signal.clone();
            let mut request = default_fetch()(request);
            let probe = probe.clone();
            let release = Release(probe.clone());
            Box::pin(poll_fn(move |context| {
                let _ = &release;
                let result = request.as_mut().poll(context);
                probe.polled(context, result.is_pending());
                result
            }))
        })
    }
    /// Acknowledge a pending poll only once per requested generation.
    fn polled(&self, context: &Context<'_>, pending: bool) {
        *self.waker.lock().unwrap() = Some(context.waker().clone());
        if pending {
            let generation = self.generation.load(Ordering::SeqCst);
            if self.acknowledged.swap(generation, Ordering::SeqCst) != generation {
                self.observations
                    .send(Observation::Polled(generation))
                    .unwrap();
            }
        }
    }
}

/// The real bundled content protocol consumes the guest's selected signal.
pub(super) struct Protocol(pub(super) Arc<Probe>);
impl<E: ProviderExecution, O: Into<StreamOptionsInput<E>>> Invoke<E, ProviderCall<E, O>>
    for Protocol
{
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, mut call: ProviderCall<E, O>) -> Self::Return {
        call.model.api = "google-generative-ai".into();
        let original = get_api_provider("google-generative-ai").unwrap();
        let stream = match call.options.unwrap().into() {
            StreamOptionsInput::Raw(mut options) => {
                *self.0.original.lock().unwrap() = options.common.signal.clone();
                options.common.fetch = Some(self.0.fetch());
                original.stream.call(
                    call.model,
                    call.context,
                    E::submission(Some(options), call.submit),
                )?
            }
            StreamOptionsInput::Simple(mut options) => {
                *self.0.original.lock().unwrap() = options.common.signal.clone();
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

/// A pending operation releases its actual HTTP future exactly once.
struct Release(Arc<Probe>);
impl Drop for Release {
    fn drop(&mut self) {
        let _ = self.0.observations.send(Observation::Dropped);
    }
}

/// The peer accepts a real request but sends no response until explicitly released.
pub(super) struct Server {
    /// Caller-supplied origin for the real native HTTP client.
    pub(super) url: String,
    /// Receipt proves the native consumer reached the local protocol peer.
    request: oneshot::Receiver<Value>,
    /// A finite response is allowed only by the removal case.
    release: std::sync::mpsc::Sender<bool>,
    /// Joined after cancellation or normal response completion.
    thread: std::thread::JoinHandle<()>,
}
impl Server {
    /// No timer or live service controls the pending response.
    pub(super) fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (observed, request) = oneshot::channel();
        let (release, permission) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let (mut peer, _) = listener.accept().unwrap();
            observed.send(read_request(&mut peer)).unwrap();
            if permission.recv().unwrap() {
                let body = "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"selected body 雪\"}],\"role\":\"model\"},\"finishReason\":\"STOP\"}]}\n\n";
                write!(peer, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                peer.flush().unwrap();
            }
        });
        Self {
            url,
            request,
            release,
            thread,
        }
    }
}

/// Abort unrelated and unselected signals only after the real request is parked at its peer.
pub(super) async fn drive(
    probe: &Probe,
    mut incoming: mpsc::UnboundedReceiver<Observation>,
    server: Server,
    signals: &[Cancellation; 3],
    action: &str,
) {
    let request = server.request.await.unwrap();
    assert_eq!(
        request["contents"],
        json!([{"role":"user","parts":[{"text":"unchanged neighbor"}]}])
    );
    assert_eq!(request["generationConfig"]["temperature"], 0.25);
    assert_eq!(
        probe.transport.lock().unwrap().is_some(),
        action != "remove"
    );
    abort_unselected(probe, &mut incoming, signals, action).await;
    if action == "remove" {
        server.release.send(true).unwrap();
    } else {
        signals[usize::from(action == "replace")].abort();
    }
    while !matches!(incoming.recv().await.unwrap(), Observation::Dropped) {}
    check_settled(probe, action, &server.release).await;
    assert!(matches!(
        incoming.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    ));
    server.thread.join().unwrap();
}

/// Each negative signal is followed by an acknowledged poll of the same pending transport.
async fn abort_unselected(
    probe: &Probe,
    incoming: &mut mpsc::UnboundedReceiver<Observation>,
    signals: &[Cancellation; 3],
    action: &str,
) {
    signals[2].abort();
    probe.checkpoint(incoming).await;
    if action != "keep" {
        signals[0].abort();
        let original = probe.original.lock().unwrap().as_ref().unwrap().clone();
        original.cancelled().await;
        probe.checkpoint(incoming).await;
    }
    if action != "replace" {
        signals[1].abort();
        probe.checkpoint(incoming).await;
    }
}

/// Cancellation drops the pending request; removing it lets the real response body finish.
async fn check_settled(probe: &Probe, action: &str, release: &std::sync::mpsc::Sender<bool>) {
    let stream = probe.stream.lock().unwrap().as_ref().unwrap().clone();
    let message = stream.result().await;
    if action == "remove" {
        assert_eq!(message.read().unwrap().content.len(), 1);
        assert_eq!(
            serde_json::to_value(&*message.read().unwrap()).unwrap()["content"][0]["text"],
            "selected body 雪"
        );
    } else {
        assert!(
            probe
                .transport
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .is_aborted()
        );
        release.send(false).unwrap();
    }
}

/// Decode only the real peer's content-length framed request.
fn read_request(peer: &mut std::net::TcpStream) -> Value {
    let mut bytes = Vec::new();
    let mut byte = [0];
    while !bytes.ends_with(b"\r\n\r\n") {
        peer.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
    }
    let headers = String::from_utf8(bytes).unwrap();
    assert!(
        headers
            .starts_with("POST /models/selected-model:streamGenerateContent?alt=sse HTTP/1.1\r\n"),
        "{headers}"
    );
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
