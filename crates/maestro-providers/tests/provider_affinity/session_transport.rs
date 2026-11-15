#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
//! Registered session sockets and HTTP fallback keep their distinct callback policies.
use maestro_models::*;
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};

/// Future and pointer erasure only; both families run the same exchange.
trait Mode: ProviderExecution {
    /// Retain the concrete admitted producer until joined.
    fn spawn(tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>) -> ProviderSpawner<Self>;
    /// Original local or thread-safe hooks count actual payload and response callbacks.
    fn common(trace: Arc<Mutex<Vec<&'static str>>>, fetch: Fetch) -> StreamOptions<Self>;
}
/// The captures remain in their original future representation over a real yield.
macro_rules! mode {
    ($mode:ty, $pointer:ident, $spawn:path) => {
        impl Mode for $mode {
            fn spawn(tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>) -> ProviderSpawner<Self> {
                $pointer::new(move |task| {
                    let task = $spawn(task);
                    tasks.lock().unwrap().push(task);
                    true
                })
            }
            fn common(trace: Arc<Mutex<Vec<&'static str>>>, fetch: Fetch) -> StreamOptions<Self> {
                let response_trace = trace.clone();
                let retained = $pointer::new(37);
                StreamOptions {
                    api_key: Some(crate::bundled::api_key("openai-codex-responses")),
                    fetch: Some(fetch),
                    on_payload: Some($pointer::new(move |mut input, _| {
                        let trace = trace.clone();
                        let retained = retained.clone();
                        Box::pin(async move {
                            trace.lock().unwrap().push("payload");
                            tokio::task::yield_now().await;
                            assert_eq!(*retained, 37);
                            input.payload["temperature"] = serde_json::json!(0.25);
                            Ok(input)
                        })
                    })),
                    on_response: Some($pointer::new(move |_, _| {
                        let trace = response_trace.clone();
                        Box::pin(async move {
                            tokio::task::yield_now().await;
                            trace.lock().unwrap().push("response");
                            Ok(())
                        })
                    })),
                    ..Default::default()
                }
            }
        }
    };
}
mode!(Local, Rc, tokio::task::spawn_local);
mode!(Threaded, Arc, tokio::spawn);
/// One real registered callback consumes the family's original raw or simple options.
fn invoke<E: Mode>(
    model: Model,
    common: StreamOptions<E>,
    simple: bool,
    spawn: ProviderSpawner<E>,
) -> crate::TestResult<AssistantMessageEventStream> {
    let context = crate::bundled::conversation()?;
    Ok(if simple {
        stream_simple_for::<E>(
            model,
            context,
            Some(SimpleStreamOptions {
                common,
                ..Default::default()
            }),
            spawn,
        )?
    } else {
        stream_for::<E>(
            model,
            context,
            Some(ProviderStreamOptions {
                common,
                ..Default::default()
            }),
            spawn,
        )?
    })
}
/// Setup chooses a successful native socket or one refused upgrade followed by SSE.
async fn run<E: Mode>(simple: bool, transport: Transport) -> crate::TestResult {
    let trace = Arc::new(Mutex::new(Vec::new()));
    let http = crate::transport::transport(vec![super::direct_response("openai-codex-responses")]);
    let (url, socket, refused) = if transport == Transport::Websocket {
        let (url, peer) = crate::response_qualification::invocation::socket::serve(vec![serde_json::json!({"type":"response.completed", "response":{"id":"affinity", "status":"completed", "output":[]}})]).await?;
        (url, Some(peer), None)
    } else {
        let peer = crate::loopback::serve(
            "HTTP/1.1 503 Unavailable\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
            vec![],
        )?;
        (peer.url.clone(), None, Some(peer))
    };
    let mut common = E::common(trace.clone(), http.fetch.clone());
    common.transport = Some(transport);
    let tasks = Arc::new(Mutex::new(Vec::new()));
    let stream = invoke::<E>(
        crate::bundled::descriptor("openai-codex-responses", &url)?,
        common,
        simple,
        E::spawn(tasks.clone()),
    )?;
    let (events, result) = crate::transport::drain(&stream).await?;
    let tasks = std::mem::take(&mut *tasks.lock().unwrap());
    assert_eq!(tasks.len(), 1);
    for task in tasks {
        task.await?;
    }
    check_result(&events, &result);
    let body = if let Some(peer) = socket {
        let request = peer.await?.map_err(|error| error.to_string())?;
        assert_eq!(*trace.lock().unwrap(), ["payload"]);
        assert_eq!(http.attempts(), 0);
        socket_body(&request)?
    } else {
        let request = refused.unwrap().finish()?;
        assert_eq!(request.header("upgrade"), Some("websocket"));
        assert_eq!(*trace.lock().unwrap(), ["payload", "response"]);
        assert_eq!(http.attempts(), 1);
        serde_json::from_slice(&http.first_request()?.1)?
    };
    assert_eq!(body["temperature"], serde_json::json!(0.25));
    Ok(())
}
/// The native peer receives the selected payload in a response.create frame.
fn socket_body(request: &HttpRequest) -> crate::TestResult<serde_json::Value> {
    let body: serde_json::Value = serde_json::from_slice(&request.body)?;
    assert_eq!(body["type"], "response.create");
    Ok(body)
}
/// Both transport paths finish a real successful stream.
fn check_result(events: &[String], result: &AssistantMessage) {
    assert_eq!(result.error_message, None);
    assert_eq!(result.stop_reason, StopReason::Stop);
    assert_eq!(events, ["start", "done"]);
}
/// Both option forms and execution families retain socket success and fallback behavior.
pub(super) async fn exercise() -> crate::TestResult {
    for simple in [false, true] {
        for transport in [Transport::Websocket, Transport::Auto] {
            run::<Local>(simple, transport.clone()).await?;
            run::<Threaded>(simple, transport).await?;
        }
    }
    Ok(())
}
