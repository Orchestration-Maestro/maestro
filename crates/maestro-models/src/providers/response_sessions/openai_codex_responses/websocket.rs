//! Native response-session socket: authenticated upgrade, full request and ordered reduction.

use super::continuation::{Request, retain, select};
use super::debug::{Mode, count_request};
use super::events::{map_codex_event, reduce};
use super::headers::build_web_socket_headers;
use super::request::{PreparedRequest, diagnostic, resolve_codex_url};
use super::sessions::{CloseReason, Identity, Lease, Socket, acquire, lock};
use super::{CodexError, OpenAICodexResponsesOptions};
use crate::arguments::json_parse::whitespace;
use crate::providers::http::{Raced, client_pairs, decode_utf8, race};
use crate::providers::json_text::{compact_members, raw_json};
use crate::{
    AssistantMessageEvent, AssistantMessageEventStream, Cancellation, DiagnosticCode,
    DiagnosticErrorInfo, Model, SharedAssistantMessage, Transport,
};
use futures_util::future::{Either, select as select_first};
use futures_util::{FutureExt, Sink, SinkExt, Stream, StreamExt};
use indexmap::IndexMap;
use serde_json::Value;
use std::borrow::Cow;
use std::sync::{Arc, LazyLock};
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite::{
    Error, Message,
    client::IntoClientRequest,
    error::ProtocolError,
    protocol::{CloseFrame, WebSocketConfig, frame::coding::CloseCode},
};
use url::Url;

/// Close code reported for a close frame that carries no status.
const NO_STATUS_CLOSE: u16 = 1005;
/// Close code reported when the peer vanished without a closing handshake.
const ABNORMAL_CLOSE: u16 = 1006;
/// Close code whose empty reason is described as an oversized message.
const MESSAGE_TOO_BIG_CLOSE: u16 = 1009;

/// The `type` of a request that names none.
static REQUEST_TYPE: LazyLock<Value> = LazyLock::new(|| Value::from("response.create"));

/// The partial message, event producer and start notification one socket operation drives.
pub(crate) struct WebSocketOutput<'a> {
    /// Message the reducer fills in.
    pub(crate) output: &'a SharedAssistantMessage,
    /// Producer receiving the start and content events.
    pub(crate) stream: &'a AssistantMessageEventStream,
    /// Called once, immediately before the first mapped event.
    pub(crate) on_start: &'a mut (dyn FnMut() + Send),
}

/// Resolve the authored endpoint, then switch an HTTP scheme to its socket counterpart.
pub(super) fn resolve_codex_web_socket_url(base_url: &str) -> Result<String, CodexError> {
    let mut url = Url::parse(&resolve_codex_url(base_url))
        .map_err(|error| CodexError::Transport(diagnostic(error.to_string())))?;
    let scheme = match url.scheme() {
        "https" => "wss",
        "http" => "ws",
        _ => return Ok(url.into()),
    };
    url.set_scheme(scheme)
        .map_err(|()| CodexError::Transport(diagnostic("Invalid WebSocket scheme")))?;
    Ok(url.into())
}

/// The request-cancelled failure.
fn aborted() -> CodexError {
    CodexError::Transport(diagnostic("Request was aborted"))
}

/// A failure of the native socket library, keeping its own wording.
fn native(error: &Error) -> CodexError {
    CodexError::Transport(diagnostic(error.to_string()))
}

/// Describe a close with its numeric code and optional reason.
pub(super) fn close_error(code: u16, reason: &str) -> CodexError {
    let reason = match reason {
        "" if code == MESSAGE_TOO_BIG_CLOSE => " message too big".to_owned(),
        "" => String::new(),
        reason => format!(" {reason}"),
    };
    let message = format!("WebSocket closed {code}{reason}");
    CodexError::Transport(DiagnosticErrorInfo {
        name: Some("WebSocketCloseError".to_owned()),
        message: message.trim_matches(whitespace).to_owned(),
        stack: None,
        code: Some(DiagnosticCode::Number(f64::from(code))),
    })
}

/// The failure a received close frame reports.
fn close_frame_error(frame: Option<CloseFrame>) -> CodexError {
    frame.map_or_else(
        || close_error(NO_STATUS_CLOSE, ""),
        |frame| close_error(frame.code.into(), frame.reason.as_str()),
    )
}

/// A library failure of a live socket; a closed socket or a reset without a closing handshake
/// is an abnormal close, and any other failure keeps the library's wording.
fn transport_error(error: &Error) -> CodexError {
    match error {
        Error::ConnectionClosed
        | Error::AlreadyClosed
        | Error::Protocol(ProtocolError::ResetWithoutClosingHandshake) => {
            close_error(ABNORMAL_CLOSE, "")
        }
        other => native(other),
    }
}

/// A message that is not a JSON document the provider selection can read.
fn invalid_json(cause: &str) -> CodexError {
    let mut error = diagnostic(format!("Invalid Codex WebSocket JSON: {cause}"));
    error.name = Some("CodexProtocolError".to_owned());
    CodexError::Protocol(error)
}

/// The message sequence of one socket, selected into reducer events.
pub(super) struct Receiver<'s, 'o, S> {
    /// Socket owned from the end of the upgrade.
    socket: &'s mut S,
    /// Caller cancellation.
    signal: Option<&'s Cancellation>,
    /// Message, producer and start notification.
    output: WebSocketOutput<'o>,
    /// A mapped event was already yielded.
    started: bool,
    /// Stop after a terminal event or failure.
    ended: bool,
    /// Failure category recovered after the reducer sees its diagnostic view.
    pub(super) failure: Option<CodexError>,
}

impl<'s, 'o, S> Receiver<'s, 'o, S>
where
    S: Stream<Item = Result<Message, Error>> + Unpin,
{
    /// Begin selecting events from an open socket.
    pub(super) fn new(
        socket: &'s mut S,
        signal: Option<&'s Cancellation>,
        output: WebSocketOutput<'o>,
    ) -> Self {
        Self {
            socket,
            signal,
            output,
            started: false,
            ended: false,
            failure: None,
        }
    }

    /// Keep the category while exposing the reducer's diagnostic item.
    fn fail(&mut self, error: CodexError) -> DiagnosticErrorInfo {
        let (CodexError::Api(info) | CodexError::Protocol(info) | CodexError::Transport(info)) =
            &error;
        let info = info.clone();
        self.failure = Some(error);
        self.ended = true;
        info
    }

    /// Announce the output before the first mapped event.
    fn start(&mut self) {
        if !std::mem::replace(&mut self.started, true) {
            (self.output.on_start)();
            self.output.stream.push(AssistantMessageEvent::Start {
                partial: Arc::clone(self.output.output),
            });
        }
    }

    /// Select one JSON document; empty text is ignored and text is never trimmed.
    fn event(&mut self, text: &str) -> Result<Option<String>, CodexError> {
        if text.is_empty() {
            return Ok(None);
        }
        let raw = raw_json(text).map_err(|error| invalid_json(&error.to_string()))?;
        if raw.get() == "null" {
            return Err(invalid_json("invalid type: null, expected a JSON object"));
        }
        let Some((text, terminal)) = map_codex_event(raw)? else {
            return Ok(None);
        };
        self.ended = terminal;
        self.start();
        Ok(Some(text))
    }

    /// Read the provider meaning of one complete message.
    fn select(&mut self, message: Message) -> Result<Option<String>, CodexError> {
        if let Some(text) = message_text(&message) {
            return self.event(&text);
        }
        match message {
            Message::Close(frame) => Err(close_frame_error(frame)),
            _ => Ok(None),
        }
    }

    /// Select the next event, cutting the socket off after a terminal event.
    pub(super) async fn next(&mut self) -> Option<Result<String, DiagnosticErrorInfo>> {
        loop {
            if self.ended {
                return None;
            }
            let received = if self.signal.is_some_and(Cancellation::is_aborted) {
                Err(aborted())
            } else {
                match race(self.socket.next(), None, self.signal).await {
                    Raced::Done(Some(Ok(message))) => self.select(message),
                    Raced::Done(Some(Err(error))) => Err(transport_error(&error)),
                    Raced::Done(None) => Err(close_error(ABNORMAL_CLOSE, "")),
                    Raced::Cancelled | Raced::TimedOut => Err(aborted()),
                }
            };
            match received {
                Ok(Some(text)) => return Some(Ok(text)),
                Ok(None) => {}
                Err(error) => return Some(Err(self.fail(error))),
            }
        }
    }
}

/// Text messages are used as sent; binary messages decode as lenient UTF-8 without one leading mark.
pub(super) fn message_text(message: &Message) -> Option<Cow<'_, str>> {
    match message {
        Message::Text(text) => Some(Cow::Borrowed(text.as_str())),
        Message::Binary(bytes) => Some(decode_utf8(bytes)),
        _ => None,
    }
}

/// The request object for a body: see [`wire_members`].
pub(super) fn wire_body(body: &Value) -> Result<String, CodexError> {
    wire_members(
        body.as_object()
            .into_iter()
            .flatten()
            .map(|(name, value)| (name.as_str(), value)),
    )
}

/// The request object: `type` holds the first non-index position with the members' own value
/// when they have one; the other members follow. Member order and number spelling are
/// those of [`compact_members`].
pub(super) fn wire_members<'a>(
    members: impl Iterator<Item = (&'a str, &'a Value)> + Clone,
) -> Result<String, CodexError> {
    let kind = members
        .clone()
        .find_map(|(name, value)| (name == "type").then_some(value))
        .unwrap_or(&REQUEST_TYPE);
    let rest = members.filter(|(name, _)| *name != "type");
    compact_members(std::iter::once(("type", kind)).chain(rest))
        .map_err(|error| CodexError::Transport(diagnostic(error.to_string())))
}

/// Send the request and reduce the response, owning every receive from the first message.
async fn exchange<S>(
    socket: &mut S,
    body: String,
    model: &Model,
    options: &OpenAICodexResponsesOptions,
    output: WebSocketOutput<'_>,
) -> Result<(), CodexError>
where
    S: Stream<Item = Result<Message, Error>> + Sink<Message, Error = Error> + Unpin,
{
    let signal = options.common.signal.as_ref();
    match race(socket.send(Message::text(body)), None, signal).await {
        Raced::Done(Ok(())) => {}
        Raced::Done(Err(error)) => return Err(transport_error(&error)),
        Raced::Cancelled | Raced::TimedOut => return Err(aborted()),
    }
    let (message, stream) = (output.output, output.stream);
    let mut receiver = Receiver::new(socket, signal, output);
    let view = futures_util::stream::unfold(&mut receiver, |receiver| async move {
        receiver.next().await.map(|event| (event, receiver))
    });
    let result = reduce(Box::pin(view), model, options, message, stream).await;
    result.map_err(|error| {
        receiver
            .failure
            .take()
            .unwrap_or(CodexError::Protocol(error))
    })
}

/// Run one request on an open socket, then send a best-effort close and release it.
pub(super) async fn run_released<S>(
    mut socket: S,
    body: String,
    model: &Model,
    options: &OpenAICodexResponsesOptions,
    output: WebSocketOutput<'_>,
) -> Result<(), CodexError>
where
    S: Stream<Item = Result<Message, Error>> + Sink<Message, Error = Error> + Unpin,
{
    let result = exchange(&mut socket, body, model, options, output).await;
    close_now(&mut socket, CloseReason::Done);
    result
}

/// Send a normal close with the reason; a close that cannot be written at once is dropped
/// with the socket.
pub(super) fn close_now<S>(socket: &mut S, reason: CloseReason)
where
    S: Sink<Message, Error = Error> + Unpin,
{
    let close = Message::Close(Some(CloseFrame {
        code: CloseCode::Normal,
        reason: reason.as_str().into(),
    }));
    let _ = socket.send(close).now_or_never();
}

/// Reject, before any connection, an endpoint with a fragment or a scheme other than `ws`/`wss`.
fn admit(url: &str) -> Result<(), CodexError> {
    let url =
        Url::parse(url).map_err(|error| CodexError::Transport(diagnostic(error.to_string())))?;
    let reason = if !matches!(url.scheme(), "ws" | "wss") {
        "expected a ws: or wss: url"
    } else if url.fragment().is_some() {
        "hash"
    } else {
        return Ok(());
    };
    let mut error = diagnostic(reason);
    error.name = Some("SyntaxError".to_owned());
    Err(CodexError::Transport(error))
}

/// Open the authenticated upgrade, unbounded in message size, unless cancelled first.
async fn connect(
    url: &str,
    headers: &mut IndexMap<String, String>,
    signal: Option<&Cancellation>,
) -> Result<Socket, CodexError> {
    let pairs = client_pairs(headers).map_err(|error| CodexError::Transport(diagnostic(error)))?;
    admit(url)?;
    let mut request = url.into_client_request().map_err(|error| native(&error))?;
    request.headers_mut().extend(pairs);
    if signal.is_some_and(Cancellation::is_aborted) {
        return Err(aborted());
    }
    let config = WebSocketConfig::default()
        .max_message_size(None)
        .max_frame_size(None);
    match race(
        Box::pin(connect_async_with_config(request, Some(config), false)),
        None,
        signal,
    )
    .await
    {
        Raced::Done(Ok((socket, _))) => Ok(socket),
        Raced::Done(Err(error)) => Err(native(&error)),
        Raced::Cancelled | Raced::TimedOut => Err(aborted()),
    }
}

/// What a finished request needs to retain its context.
pub(super) struct Retention<'a> {
    /// The full body that was prepared.
    pub(super) body: &'a Value,
    /// The reduced response.
    pub(super) output: &'a SharedAssistantMessage,
    /// The model that produced it.
    pub(super) model: &'a Model,
    /// The cached-context transport was selected.
    pub(super) cached: bool,
    /// Caller cancellation.
    pub(super) signal: Option<&'a Cancellation>,
}

/// Send the close an explicit session close asked for, then report how the peer ended the socket.
async fn close_socket(
    socket: &mut Socket,
    reason: CloseReason,
    signal: Option<&Cancellation>,
) -> CodexError {
    let frame = Message::Close(Some(CloseFrame {
        code: CloseCode::Normal,
        reason: reason.as_str().into(),
    }));
    if let Raced::Cancelled | Raced::TimedOut = race(socket.send(frame), None, signal).await {
        return aborted();
    }
    loop {
        match race(socket.next(), None, signal).await {
            Raced::Done(Some(Ok(Message::Close(frame)))) => return close_frame_error(frame),
            Raced::Done(Some(Ok(_))) => {}
            Raced::Done(Some(Err(error))) => return transport_error(&error),
            Raced::Done(None) => return close_error(ABNORMAL_CLOSE, ""),
            Raced::Cancelled | Raced::TimedOut => return aborted(),
        }
    }
}

/// Run one request on the leased socket; an explicit close of the session ends it with the
/// close the peer answers with, keeping the output reduced so far.
async fn run_leased(
    lease: &mut Lease,
    wire: String,
    model: &Model,
    options: &OpenAICodexResponsesOptions,
    output: WebSocketOutput<'_>,
) -> Result<(), CodexError> {
    let closed = lease.closed();
    let finished = {
        let operation = exchange(&mut lease.held.socket, wire, model, options, output);
        match select_first(Box::pin(operation), Box::pin(closed)).await {
            Either::Left((result, _)) => Ok(result),
            Either::Right((reason, _)) => Err(reason),
        }
    };
    match finished {
        Ok(result) => result,
        Err(reason) => {
            let signal = options.common.signal.as_ref();
            Err(close_socket(&mut lease.held.socket, reason, signal).await)
        }
    }
}

/// Settle the lease after a request: a healthy socket returns to the cache, a completed cached
/// request retains its context, and a failure clears the context and closes the socket.
pub(super) fn conclude(
    lease: Lease,
    result: Result<(), CodexError>,
    retention: &Retention<'_>,
) -> Result<(), CodexError> {
    let outcome = result.and_then(|()| {
        if retention.signal.is_some_and(Cancellation::is_aborted) {
            return Ok(false);
        }
        if let Some(slot) = lease.continuation().filter(|_| retention.cached) {
            retain(slot, retention.body, retention.output, retention.model)?;
        }
        Ok(true)
    });
    match outcome {
        Ok(true) => lease.keep(),
        Ok(false) => {}
        Err(_) => {
            if let Some(slot) = lease.continuation() {
                *lock(slot) = None;
            }
        }
    }
    outcome.map(drop)
}

/// Send one request over the session's cached socket or a new one: the input suffix with the
/// previous response identifier when the cached-context transport is selected and the retained
/// context matches, otherwise the full request.
pub(crate) async fn process_web_socket_stream(
    prepared: &PreparedRequest,
    model: &Arc<Model>,
    options: &OpenAICodexResponsesOptions,
    output: WebSocketOutput<'_>,
    request_id: &str,
) -> Result<(), CodexError> {
    let mut headers =
        build_web_socket_headers(&prepared.headers, request_id).map_err(CodexError::Transport)?;
    let url = resolve_codex_web_socket_url(&model.base_url)?;
    let full = Request::full(&prepared.body)?;
    let signal = options.common.signal.as_ref();
    let session = options
        .common
        .session_id
        .as_deref()
        .filter(|session| !session.is_empty());
    let identity = Identity::new(&url, &headers);
    let mut lease = acquire(session, identity, connect(&url, &mut headers, signal)).await?;
    let cached = matches!(
        options.common.transport,
        Some(Transport::WebsocketCached | Transport::Auto)
    );
    let request = lease
        .continuation()
        .filter(|_| cached)
        .and_then(|slot| select(slot, &prepared.body))
        .unwrap_or(full);
    if let Some(session) = session {
        let mode = Mode {
            reused: lease.reused,
            cached,
        };
        count_request(session, &request, mode);
    }
    let retention = Retention {
        body: &prepared.body,
        output: output.output,
        model,
        cached,
        signal,
    };
    let result = run_leased(&mut lease, request.wire, model, options, output).await;
    conclude(lease, result, &retention)
}
