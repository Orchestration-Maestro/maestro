//! A loopback chat-completions provider with completion-based shutdown.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

/// Native HTTP fixture retaining each actual request.
pub struct Provider {
    /// Root of the controlled chat-completions API.
    pub base: String,
    /// Listener address used to release a blocked accept during teardown.
    address: std::net::SocketAddr,
    /// Requests seen by the provider.
    pub requests: Arc<Mutex<Vec<Value>>>,
    /// Whether teardown requested EOF.
    stop: Arc<AtomicBool>,
    /// Serving thread, joined exactly once.
    worker: Option<JoinHandle<()>>,
}
impl Provider {
    /// Listen before starting the executable; no external credentials are involved.
    pub fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (seen, stopping) = (Arc::clone(&requests), Arc::clone(&stop));
        let worker = std::thread::spawn(move || serve(&listener, &seen, &stopping));
        Self {
            base: format!("http://{address}/v1"),
            address,
            requests,
            stop,
            worker: Some(worker),
        }
    }
}
impl Drop for Provider {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        drop(TcpStream::connect(self.address));
        self.worker.take().unwrap().join().unwrap();
    }
}

/// Read headers through their blank line, then exactly the declared JSON body.
pub(crate) fn request(stream: &mut TcpStream) -> Value {
    let mut reader = BufReader::new(stream);
    let mut length = 0;
    loop {
        let mut line = String::new();
        assert_ne!(reader.read_line(&mut line).unwrap(), 0);
        if line == "\r\n" {
            break;
        }
        if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            length = value.trim().parse().unwrap();
        }
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Answer actual requests until the owner's wake-up connection signals teardown.
fn serve(listener: &TcpListener, seen: &Mutex<Vec<Value>>, stopping: &AtomicBool) {
    loop {
        let (mut stream, _) = listener.accept().unwrap();
        if stopping.load(Ordering::Relaxed) {
            break;
        }
        let request = request(&mut stream);
        let text = if request.to_string().contains("root-context-Ω") {
            "root-context-Ω"
        } else {
            "test123"
        };
        let chunk = chunk(&request, text);
        seen.lock().unwrap().push(request);
        let events = format!("data: {chunk}\n\ndata: [DONE]\n\n");
        write!(stream, "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{events}", events.len()).unwrap();
    }
}

/// Emits one harmless tool call only when the newest user message requests that scenario.
fn chunk(request: &Value, text: &str) -> Value {
    let dangerous = request["messages"]
        .as_array()
        .and_then(|messages| messages.last())
        .is_some_and(|message| {
            message["role"] == "user" && message["content"].to_string().contains("rpc-demo-danger")
        });
    if dangerous {
        return json!({"id":"controlled", "choices":[{"index":0,"delta":{"tool_calls":[{
            "index":0,"id":"rpc-demo-tool","type":"function","function":{"name":"bash","arguments":"{\"command\":\"echo sudo\"}"}
        }]},"finish_reason":"tool_calls"}]});
    }
    json!({"id":"controlled", "choices":[{"index":0,"delta":{"content":text},"finish_reason":"stop"}], "usage":{"prompt_tokens":20,"completion_tokens":4,"total_tokens":24}})
}
