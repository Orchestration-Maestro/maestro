//! Native browser socket bindings using retained event subscriptions.
use super::{Event, Queue, next, push};
use crate::browser::event_subscription::EventSubscription;
use maestro_app::{ConnectWebSocket, WebSocket, WebSocketConnection, WebSocketFuture};
use std::{cell::RefCell, io, rc::Rc, sync::Arc};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::{Event as NativeEvent, MessageEvent};

/// Retained browser callback type.
type Callback = Closure<dyn FnMut(NativeEvent)>;
/// Browser connection and its weakly captured event queue.
struct BrowserSocket {
    /// Native platform connection.
    socket: web_sys::WebSocket,
    /// Notifications owned by this connection alone.
    queue: Rc<RefCell<Queue>>,
    /// Exact retained registrations.
    subscriptions: Vec<EventSubscription<Callback>>,
}
/// Convert a platform failure without including request credentials.
fn native_error(value: &JsValue) -> io::Error {
    io::Error::other(
        value
            .as_string()
            .unwrap_or_else(|| "WebSocket operation failed".into()),
    )
}
impl BrowserSocket {
    /// Install all handlers synchronously before awaiting the connection.
    fn new(url: &str) -> io::Result<Self> {
        let socket = web_sys::WebSocket::new(url).map_err(|error| native_error(&error))?;
        socket.set_binary_type(web_sys::BinaryType::Arraybuffer);
        let mut connection = Self {
            socket,
            queue: Rc::new(RefCell::new(Queue::default())),
            subscriptions: Vec::new(),
        };
        for name in ["open", "message", "error", "close"] {
            let weak = Rc::downgrade(&connection.queue);
            let callback = Closure::new(move |event: NativeEvent| {
                push(&weak, notification(name, event));
            });
            connection.subscriptions.push(
                EventSubscription::new(connection.socket.clone().into(), name, callback)
                    .map_err(|error| native_error(&error))?,
            );
        }
        Ok(connection)
    }
}
/// Convert complete browser messages without JSON interpretation.
fn notification(name: &str, event: NativeEvent) -> Event {
    match name {
        "open" => Event::Open,
        "close" => Event::Close,
        "message" => {
            let data = event.unchecked_into::<MessageEvent>().data();
            if let Some(text) = data.as_string() {
                return Event::Data(text.into_bytes());
            }
            if data.is_instance_of::<js_sys::ArrayBuffer>() {
                return Event::Data(js_sys::Uint8Array::new(&data).to_vec());
            }
            Event::Error(io::Error::other("Unsupported WebSocket data message"))
        }
        _ => Event::Error(io::Error::other("WebSocket connection failed")),
    }
}
impl WebSocketConnection for BrowserSocket {
    fn send(&mut self, bytes: Vec<u8>) -> WebSocketFuture<'_, io::Result<()>> {
        Box::pin(async move {
            let text = String::from_utf8(bytes).map_err(io::Error::other)?;
            self.socket
                .send_with_str(&text)
                .map_err(|error| native_error(&error))
        })
    }
    fn receive(&mut self) -> WebSocketFuture<'_, io::Result<Option<Vec<u8>>>> {
        Box::pin(async move { receive(&self.queue).await })
    }
    fn close(&mut self) {
        self.subscriptions.clear();
        let _ = self.socket.close();
    }
}
/// Select data or terminal notifications after the open handshake.
async fn receive(queue: &Rc<RefCell<Queue>>) -> io::Result<Option<Vec<u8>>> {
    loop {
        match next(queue).await {
            Event::Data(bytes) => return Ok(Some(bytes)),
            Event::Error(error) => return Err(error),
            Event::Close => return Ok(None),
            Event::Open => {}
        }
    }
}
impl Drop for BrowserSocket {
    fn drop(&mut self) {
        self.close();
    }
}
/// Browser-local factory; custom upgrade headers are unsupported by the platform.
#[must_use]
pub fn browser_web_socket() -> ConnectWebSocket {
    Arc::new(|request| {
        Box::pin(async move {
            if !request.headers.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Browser WebSocket headers are unsupported",
                ));
            }
            let connection = BrowserSocket::new(&request.url)?;
            match next(&connection.queue).await {
                Event::Open => Ok(Box::new(connection) as WebSocket),
                Event::Error(error) => Err(error),
                Event::Close | Event::Data(_) => {
                    Err(io::Error::other("WebSocket connection closed"))
                }
            }
        })
    })
}
