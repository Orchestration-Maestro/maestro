//! Loopback conversations and peers shared by the cached-socket witnesses.

use super::super::request::{PreparedRequest, prepare_request};
use super::socket_transport::{Outcome, Setup, operate_prepared, setup};
use crate::{Context, SharedAssistantMessage, Transport};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::sync::atomic::AtomicUsize;
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;

/// One conversation against a loopback endpoint; each turn prepares its own request.
pub(super) struct Conversation {
    /// Descriptor, options and the first prepared request.
    pub(super) setup: Setup,
    /// History sent by the next turn.
    context: Context,
}

impl Conversation {
    /// Start a conversation holding the controlled first user message.
    pub(super) async fn new(
        base_url: &str,
        session: Option<&str>,
        transport: Option<Transport>,
    ) -> Self {
        let setup = setup(base_url, |options| {
            options.common.session_id = session.map(str::to_owned);
            options.common.transport = transport;
        })
        .await;
        Self {
            setup,
            context: super::controlled_context(),
        }
    }

    /// Append a message given as JSON.
    fn push(&mut self, message: Value) {
        let mut context = serde_json::to_value(&self.context).unwrap();
        context["messages"].as_array_mut().unwrap().push(message);
        self.context = serde_json::from_value(context).unwrap();
    }

    /// Append the reduced reply.
    pub(super) fn reply(&mut self, reply: &SharedAssistantMessage) {
        let reply = serde_json::to_value(reply.read().unwrap().clone()).unwrap();
        self.push(reply);
    }

    /// Append the reduced reply and the next user message.
    pub(super) fn follow(&mut self, reply: &SharedAssistantMessage, next: &str) {
        self.reply(reply);
        self.user(next);
    }

    /// Start over with a single different user message.
    pub(super) fn restart(&mut self, text: &str) {
        self.context = serde_json::from_value(json!({"messages": []})).unwrap();
        self.user(text);
    }

    /// Append a user message.
    pub(super) fn user(&mut self, text: &str) {
        self.push(json!({"role": "user", "content": text, "timestamp": 1}));
    }

    /// Prepare the current history.
    pub(super) async fn prepared(&self) -> PreparedRequest {
        prepare_request(
            &self.setup.model,
            &self.context,
            &self.setup.options,
            "maestro (fixture)",
        )
        .await
        .unwrap()
    }

    /// Prepare and send the current history with request identifier `req`.
    pub(super) async fn send(&self) -> Outcome {
        let prepared = self.prepared().await;
        operate_prepared(&self.setup, &prepared, "req", &AtomicUsize::new(0)).await
    }
}

/// Events of a completed one-message response named `id`; without one the response has none.
pub(super) fn response_events(id: Option<&str>, text: &str) -> Vec<String> {
    let response = id.map_or_else(|| json!({}), |id| json!({"id": id}));
    let mut completed = response.clone();
    completed["status"] = json!("completed");
    [
        json!({"type":"response.created","response":response}),
        json!({"type":"response.output_item.added","item":{"type":"message","id":"m1","role":"assistant","status":"in_progress","content":[]}}),
        json!({"type":"response.content_part.added","part":{"type":"output_text","text":""}}),
        json!({"type":"response.output_text.delta","delta":text}),
        json!({"type":"response.output_item.done","item":{"type":"message","id":"m1","role":"assistant","status":"completed","content":[{"type":"output_text","text":text}]}}),
        json!({"type":"response.completed","response":completed}),
    ]
    .iter()
    .map(Value::to_string)
    .collect()
}

/// Read the request that arrives next on the peer's socket.
pub(super) async fn read_request(socket: &mut WebSocketStream<TcpStream>) -> Value {
    let message = socket.next().await.unwrap().unwrap();
    serde_json::from_str(message.to_text().unwrap()).unwrap()
}

/// Send a completed response.
pub(super) async fn respond(socket: &mut WebSocketStream<TcpStream>, id: Option<&str>, text: &str) {
    for event in response_events(id, text) {
        socket.send(Message::text(event)).await.unwrap();
    }
}

/// Read the next request and answer it with a completed response named `id`.
pub(super) async fn answer(socket: &mut WebSocketStream<TcpStream>, id: &str, text: &str) -> Value {
    let request = read_request(socket).await;
    respond(socket, Some(id), text).await;
    request
}
