//! Lookup-handle ownership across a real dispatcher connection.

use super::*;
use crate::browser_driver::{
    connection::Dispatcher,
    inputs,
    tests::{INVENTORY, block_on},
};
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};

/// A returned remote handle is released once even when native lookup fails.
#[test]
fn sandbox_inspection_disposes_lookup_handles() {
    block_on(async {
        for outcome in ["success", "missing", "close"] {
            let (mut dispatcher, server) =
                connect_peer(move |socket| scripted(socket, outcome)).await;
            let mut lines = Vec::new();
            let expected = inputs::inventory(INVENTORY).unwrap().expected_error;
            let mut page = Page::open(&mut dispatcher, "lookup", &mut lines, &expected)
                .await
                .unwrap();
            let context = Context {
                session: page.session.clone(),
                id: 1,
                frame: "child".into(),
            };
            page.contexts.insert("child".into(), context.clone());
            let result = page.selected(&context, "mount").await;
            let expected_error = if outcome == "missing" {
                "mounted frame not found"
            } else {
                "page closed"
            };
            if outcome == "success" {
                assert_eq!(result.unwrap().frame, "child");
            } else {
                let error = result.unwrap_err().to_string();
                assert!(error.contains(expected_error), "{error}");
            }
            page.close().await.unwrap();
            let releases = server.await.unwrap();
            assert_eq!(
                releases,
                if outcome == "missing" {
                    Vec::new()
                } else {
                    vec![json!({"objectId":"lookup-handle"})]
                },
                "{outcome}"
            );
        }
    });
}

/// Respond to native operations while retaining only remote-handle releases.
async fn scripted(mut socket: WebSocketStream<TcpStream>, outcome: &str) -> Vec<Value> {
    let mut releases = Vec::new();
    while let Some(message) = socket.next().await {
        let message = message.unwrap();
        let call: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
        let method = call["method"].as_str().unwrap();
        let result = match method {
            "Target.createBrowserContext" => json!({"browserContextId":"context"}),
            "Target.createTarget" => json!({"targetId":"target"}),
            "Target.attachToTarget" => json!({"sessionId":"session"}),
            "Runtime.evaluate" if outcome == "missing" => {
                json!({"result":{"type":"object","subtype":"null","value":null}})
            }
            "Runtime.evaluate" => json!({"result":{"type":"object","objectId":"lookup-handle"}}),
            "DOM.describeNode" => json!({"node":{"frameId":"child"}}),
            "Runtime.releaseObject" => {
                assert_eq!(call["sessionId"], "session");
                releases.push(call["params"].clone());
                json!({})
            }
            "Target.closeTarget" | "Target.disposeBrowserContext" => json!({}),
            other => panic!("unexpected command {other}"),
        };
        let response = if outcome == "close" && method == "DOM.describeNode" {
            json!({"id":call["id"],"error":{"code":-32000,"message":"page closed"}})
        } else {
            json!({"id":call["id"],"result":result})
        };
        socket
            .send(Message::text(response.to_string()))
            .await
            .unwrap();
        if method == "Target.disposeBrowserContext" {
            break;
        }
    }
    releases
}

/// Running and debugger-paused iframe attachments both expose their default world to inspection.
#[test]
fn sandbox_inspection_tracks_running_and_paused_iframe_renderers() {
    block_on(async {
        for waiting in [false, true] {
            inspect_attached_iframe(waiting).await;
        }
    });
}

/// Inspect through real attachment and context events, without pre-registering the child world.
async fn inspect_attached_iframe(waiting: bool) {
    let (mut dispatcher, peer) = connect_peer(move |socket| attached_iframe(socket, waiting)).await;
    let mut lines = Vec::new();
    let expected = inputs::inventory(INVENTORY).unwrap().expected_error;
    let mut page = Page::open(&mut dispatcher, "attached", &mut lines, &expected)
        .await
        .unwrap();
    let root = Context {
        session: page.session.clone(),
        id: 1,
        frame: "root".into(),
    };
    page.contexts.insert("root".into(), root.clone());

    page.evaluate("mount()").await.unwrap();
    let result = page.inspect(&root, "probe:\"雪]", "state").await;
    page.close().await.unwrap();
    peer.await.unwrap();
    result.unwrap();
}

/// Build the iframe attachment and default-world events for the native peer.
fn iframe_events(waiting: bool) -> [Value; 2] {
    let attached = json!({
        "method":"Target.attachedToTarget","sessionId":"session",
        "params":{"sessionId":"child-session","waitingForDebugger":waiting,
            "targetInfo":{"targetId":"child","type":"iframe","browserContextId":"context",
                "title":"","url":"about:srcdoc","attached":true,"canAccessOpener":false}}
    });
    let context_created = json!({
        "method":"Runtime.executionContextCreated","sessionId":"child-session",
        "params":{"context":{"id":2,"origin":"null","name":"","uniqueId":"child-world",
            "auxData":{"isDefault":true,"frameId":"child"}}}
    });
    [attached, context_created]
}

/// Emit a native iframe attachment, then its default world when Runtime is enabled.
async fn attached_iframe(mut socket: WebSocketStream<TcpStream>, waiting: bool) {
    let [attached, context_created] = iframe_events(waiting);
    let mut inspected = false;
    while let Some(message) = socket.next().await {
        let message = message.unwrap();
        let call: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
        let method = call["method"].as_str().unwrap();
        let result = match method {
            "Target.createBrowserContext" => json!({"browserContextId":"context"}),
            "Target.createTarget" => json!({"targetId":"target"}),
            "Target.attachToTarget" => json!({"sessionId":"session"}),
            "Runtime.evaluate" if call["params"]["expression"] == "mount()" => {
                socket
                    .send(Message::text(attached.to_string()))
                    .await
                    .unwrap();
                json!({"result":{"value":true}})
            }
            "Runtime.enable" => {
                assert_eq!(call["sessionId"], "child-session");
                socket
                    .send(Message::text(context_created.to_string()))
                    .await
                    .unwrap();
                json!({})
            }
            "Runtime.evaluate" if call["params"]["expression"] == scripts::STATE => {
                assert_eq!(call["sessionId"], "child-session");
                assert_eq!(call["params"]["contextId"], 2);
                inspected = true;
                json!({"result":{"value":{}}})
            }
            "Runtime.evaluate" => json!({"result":{"type":"object","objectId":"lookup-handle"}}),
            "DOM.describeNode" => json!({"node":{"frameId":"child"}}),
            "Page.enable"
            | "Inspector.enable"
            | "Runtime.addBinding"
            | "Page.addScriptToEvaluateOnNewDocument"
            | "Target.setAutoAttach"
            | "Runtime.runIfWaitingForDebugger"
            | "Runtime.releaseObject"
            | "Target.closeTarget"
            | "Target.disposeBrowserContext" => json!({}),
            other => panic!("unexpected command {other}"),
        };
        socket
            .send(Message::text(
                json!({"id":call["id"],"result":result}).to_string(),
            ))
            .await
            .unwrap();
        if method == "Target.disposeBrowserContext" {
            break;
        }
    }
    assert!(
        inspected,
        "iframe state was not evaluated; paused={waiting}"
    );
}

/// A host call that arrives during an iframe's setup is answered after its default world is known.
#[test]
fn host_calls_wait_for_attached_iframe_initialization() {
    block_on(async {
        assert_eq!(
            gated(Gate {
                early: &[1],
                late: None
            })
            .await,
            [reply(1)]
        );
    });
}

/// A host call that arrives while queued calls are being released waits behind them.
#[test]
fn deferred_host_calls_keep_arrival_order_while_released() {
    block_on(async {
        assert_eq!(
            gated(Gate {
                early: &[1, 2],
                late: Some(3)
            })
            .await,
            [reply(1), reply(2), reply(3)]
        );
    });
}

/// The host calls the scripted peer sends around an iframe's setup.
#[derive(Clone, Copy)]
struct Gate {
    /// Calls sent while the iframe's setup is open.
    early: &'static [i64],
    /// Call sent while the first queued call is being released.
    late: Option<i64>,
}

/// Run one gated scenario and return every reply evaluated in the root world.
async fn gated(gate: Gate) -> Vec<String> {
    let (mut dispatcher, peer) = connect_peer(move |socket| gated_iframe(socket, gate)).await;
    let mut lines = Vec::new();
    let expected = inputs::inventory(INVENTORY).unwrap().expected_error;
    let mut page = Page::open(&mut dispatcher, "gated", &mut lines, &expected)
        .await
        .unwrap();
    page.initialize().await.unwrap();
    page.evaluate(HOLD).await.unwrap();
    page.close().await.unwrap();
    peer.await.unwrap()
}

/// The root-world evaluation that answers sandbox inspection call `id`.
fn reply(id: i64) -> String {
    format!(r#"__maestroReply({id},{{"value":"right"}},null)"#)
}

/// Build the page's inspection call `id`.
fn binding_called(id: i64) -> Value {
    let payload = json!({"id":id,"name":"inspectSandbox","args":["probe:\"雪]","state"]});
    json!({
        "method":"Runtime.bindingCalled","sessionId":"session",
        "params":{"name":"__maestroDriver","executionContextId":1,"payload":payload.to_string()}
    })
}

/// The evaluation the scripted peer holds open while the iframe attaches.
const HOLD: &str = "hold()";

/// Serve one scripted peer and connect a dispatcher to it.
async fn connect_peer<T, F, Fut>(script: F) -> (Dispatcher, tokio::task::JoinHandle<T>)
where
    F: FnOnce(WebSocketStream<TcpStream>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let peer = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        script(tokio_tungstenite::accept_async(stream).await.unwrap()).await
    });
    (
        Dispatcher::connect(&format!("ws://{address}"))
            .await
            .unwrap(),
        peer,
    )
}

/// Send one scripted record to the driver.
async fn send(socket: &mut WebSocketStream<TcpStream>, record: Value) {
    socket
        .send(Message::text(record.to_string()))
        .await
        .unwrap();
}

/// Read one driver command.
async fn receive(socket: &mut WebSocketStream<TcpStream>) -> Option<Value> {
    let message = socket.next().await?.unwrap();
    Some(serde_json::from_str(message.to_text().unwrap()).unwrap())
}

/// Answer the setup and teardown commands every scripted peer shares.
fn shared_reply(call: &Value) -> Option<Value> {
    Some(match call["method"].as_str()? {
        "Target.createBrowserContext" => json!({"browserContextId":"context"}),
        "Target.createTarget" => json!({"targetId":"target"}),
        "Target.attachToTarget" if call["params"]["targetId"] == "target" => {
            json!({"sessionId":"session"})
        }
        "Target.attachToTarget" => json!({"sessionId":"caller"}),
        "Page.getFrameTree" => json!({"frameTree":{"frame":{"id":"root"}}}),
        "Page.enable"
        | "Inspector.enable"
        | "Runtime.addBinding"
        | "Page.addScriptToEvaluateOnNewDocument"
        | "Target.setAutoAttach"
        | "Runtime.runIfWaitingForDebugger"
        | "Runtime.releaseObject"
        | "Emulation.setDeviceMetricsOverride"
        | "Target.closeTarget"
        | "Target.disposeBrowserContext" => json!({}),
        _ => return None,
    })
}

/// Build a default-world announcement for one renderer.
fn context_created(session: &str, id: i64, frame: &str) -> Value {
    json!({
        "method":"Runtime.executionContextCreated","sessionId":session,
        "params":{"context":{"id":id,"origin":"null","name":"","uniqueId":frame,
            "auxData":{"isDefault":true,"frameId":frame}}}
    })
}

/// Attach an iframe while the root evaluation is open and call the host before its setup ends.
///
/// Returns every reply the driver evaluated in the root world.
async fn gated_iframe(mut socket: WebSocketStream<TcpStream>, gate: Gate) -> Vec<String> {
    let attached = json!({
        "method":"Target.attachedToTarget","sessionId":"session",
        "params":{"sessionId":"child-session","waitingForDebugger":false,
            "targetInfo":{"targetId":"child","type":"iframe","browserContextId":"context",
                "title":"","url":"about:srcdoc","attached":true,"canAccessOpener":false}}
    });
    let (mut held, mut pending, mut replies) = (None, gate, Vec::new());
    while let Some(call) = receive(&mut socket).await {
        let method = call["method"].as_str().unwrap();
        let expression = call["params"]["expression"].as_str().unwrap_or_default();
        if method == "Runtime.evaluate" && expression == HOLD {
            held = Some(call["id"].clone());
            send(&mut socket, attached.clone()).await;
            continue;
        }
        for id in due_calls(&call, &mut pending) {
            send(&mut socket, binding_called(id)).await;
        }
        if method == "Runtime.enable" {
            send(&mut socket, enabled_context(&call)).await;
        }
        let result = gated_reply(&call, &mut replies);
        send(&mut socket, json!({"id":call["id"],"result":result})).await;
        if expression.starts_with("__maestroReply(")
            && replies.len() == gate.early.len() + usize::from(gate.late.is_some())
        {
            let held = held.take().expect("the root evaluation is held");
            send(
                &mut socket,
                json!({"id":held,"result":{"result":{"value":null}}}),
            )
            .await;
        }
        if method == "Target.disposeBrowserContext" {
            break;
        }
    }
    replies
}

/// Take the host calls that `call` triggers: the early ones when the iframe starts its setup, the
/// late one when the first queued call is released.
fn due_calls(call: &Value, pending: &mut Gate) -> Vec<i64> {
    let method = call["method"].as_str();
    if method == Some("Page.enable") && call["sessionId"] == "child-session" {
        return std::mem::take(&mut pending.early).to_vec();
    }
    if method == Some("Runtime.evaluate") && call["params"]["contextId"] == 1 {
        return pending.late.take().into_iter().collect();
    }
    Vec::new()
}

/// Announce the default world of the renderer that just enabled its runtime.
fn enabled_context(call: &Value) -> Value {
    if call["sessionId"] == "session" {
        context_created("session", 1, "root")
    } else {
        context_created("child-session", 2, "child")
    }
}

/// Answer one command of the gated scenario, recording each reply evaluated in the root world.
fn gated_reply(call: &Value, replies: &mut Vec<String>) -> Value {
    let method = call["method"].as_str().unwrap();
    let expression = call["params"]["expression"].as_str().unwrap_or_default();
    match method {
        "Runtime.enable" => json!({}),
        "Runtime.evaluate" if expression.starts_with("__maestroReply(") => {
            replies.push(expression.to_owned());
            json!({"result":{"value":null}})
        }
        "Runtime.evaluate" if call["params"]["contextId"] == 2 => {
            assert_eq!(expression, scripts::STATE);
            json!({"result":{"value":{"value":"right"}}})
        }
        "Runtime.evaluate" if call["params"]["contextId"] == 1 => {
            json!({"result":{"type":"object","objectId":"lookup-handle"}})
        }
        "Runtime.evaluate" => json!({"result":{"value":null}}),
        "DOM.describeNode" => json!({"node":{"frameId":"child"}}),
        _ => shared_reply(call).unwrap_or_else(|| panic!("unexpected command {method}")),
    }
}

/// A session the caller attached itself receives no commands from the page.
#[test]
fn caller_iframe_sessions_stay_with_the_caller() {
    block_on(async {
        let (mut dispatcher, peer) = connect_peer(caller_session).await;
        let mut lines = Vec::new();
        let expected = inputs::inventory(INVENTORY).unwrap().expected_error;
        let mut page = Page::open(&mut dispatcher, "caller", &mut lines, &expected)
            .await
            .unwrap();
        let attached = page
            .send(
                None,
                "Target.attachToTarget",
                json!({"targetId":"child","flatten":true}),
            )
            .await
            .unwrap();
        assert_eq!(attached["sessionId"], "caller");
        page.evaluate("1").await.unwrap();
        page.close().await.unwrap();
        assert_eq!(peer.await.unwrap(), Vec::<String>::new());
    });
}

/// Announce the caller's own session in the page's browser context, session-less, and record
/// every command addressed to it.
async fn caller_session(mut socket: WebSocketStream<TcpStream>) -> Vec<String> {
    let announced = json!({
        "method":"Target.attachedToTarget",
        "params":{"sessionId":"caller","waitingForDebugger":false,
            "targetInfo":{"targetId":"child","type":"iframe","browserContextId":"context",
                "title":"","url":"about:srcdoc","attached":true,"canAccessOpener":false}}
    });
    let mut received = Vec::new();
    while let Some(call) = receive(&mut socket).await {
        let method = call["method"].as_str().unwrap();
        let result = if call["sessionId"] == "caller" {
            received.push(method.to_owned());
            json!({})
        } else if method == "Runtime.evaluate" {
            json!({"result":{"value":1}})
        } else {
            shared_reply(&call).unwrap_or_else(|| panic!("unexpected command {method}"))
        };
        send(&mut socket, json!({"id":call["id"],"result":result})).await;
        if method == "Target.attachToTarget" && call["params"]["targetId"] == "child" {
            send(&mut socket, announced.clone()).await;
        }
        if method == "Target.disposeBrowserContext" {
            break;
        }
    }
    received
}

/// An iframe removed while its setup commands are in flight settles its own host call and leaves
/// the awaited command alone.
#[test]
fn iframe_removed_during_setup_settles_its_host_call() {
    block_on(async {
        let (mut dispatcher, peer) = connect_peer(removed_during_setup).await;
        let mut lines = Vec::new();
        let expected = inputs::inventory(INVENTORY).unwrap().expected_error;
        let mut page = Page::open(&mut dispatcher, "removed", &mut lines, &expected)
            .await
            .unwrap();
        page.initialize().await.unwrap();
        page.evaluate(HOLD).await.unwrap();
        page.close().await.unwrap();
        assert_eq!(
            peer.await.unwrap(),
            [format!(
                "__maestroReply(1,null,{})",
                json!("mounted frame not found probe:\"雪]")
            )]
        );
    });
}

/// Attach an iframe during the held evaluation, remove it as its first setup command arrives and
/// call the host before answering that command, which Chromium never does.
///
/// Returns every reply the driver evaluated in the root world.
async fn removed_during_setup(mut socket: WebSocketStream<TcpStream>) -> Vec<String> {
    let [attached, _] = iframe_events(false);
    let detached = json!({
        "method":"Target.detachedFromTarget","sessionId":"session",
        "params":{"sessionId":"child-session"}
    });
    let (mut held, mut replies) = (None, Vec::new());
    while let Some(call) = receive(&mut socket).await {
        let method = call["method"].as_str().unwrap();
        let expression = call["params"]["expression"].as_str().unwrap_or_default();
        if method == "Runtime.evaluate" && expression == HOLD {
            held = Some(call["id"].clone());
            send(&mut socket, attached.clone()).await;
            continue;
        }
        if method == "Page.enable" && call["sessionId"] == "child-session" {
            send(&mut socket, detached.clone()).await;
            send(&mut socket, binding_called(1)).await;
            continue;
        }
        let result = match method {
            "Runtime.enable" => {
                send(&mut socket, context_created("session", 1, "root")).await;
                json!({})
            }
            "Runtime.evaluate" if expression.starts_with("__maestroReply(") => {
                replies.push(expression.to_owned());
                json!({"result":{"value":null}})
            }
            "Runtime.evaluate" => json!({"result":{"type":"object","subtype":"null","value":null}}),
            _ => shared_reply(&call).unwrap_or_else(|| panic!("unexpected command {method}")),
        };
        send(&mut socket, json!({"id":call["id"],"result":result})).await;
        if let Some(held) = held.take_if(|_| !replies.is_empty()) {
            send(
                &mut socket,
                json!({"id":held,"result":{"result":{"value":null}}}),
            )
            .await;
        }
        if method == "Target.disposeBrowserContext" {
            break;
        }
    }
    replies
}

/// A host call whose caller renderer is gone has no reply to deliver, so closing the page succeeds.
#[test]
fn host_call_from_a_closing_page_does_not_fail_close() {
    block_on(async {
        let (mut dispatcher, peer) = connect_peer(caller_removed_while_closing).await;
        let mut lines = Vec::new();
        let expected = inputs::inventory(INVENTORY).unwrap().expected_error;
        let mut page = Page::open(&mut dispatcher, "closing", &mut lines, &expected)
            .await
            .unwrap();
        page.initialize().await.unwrap();
        page.close().await.unwrap();
        peer.await.unwrap();
    });
}

/// Call the host while the target closes, then remove the caller's renderer before its lookup is
/// answered, as Chromium drops commands of a destroyed renderer. Replies to that renderer are
/// refused.
async fn caller_removed_while_closing(mut socket: WebSocketStream<TcpStream>) {
    let detached = json!({"method":"Target.detachedFromTarget","params":{"sessionId":"session"}});
    let mut close = None;
    while let Some(call) = receive(&mut socket).await {
        let method = call["method"].as_str().unwrap();
        let expression = call["params"]["expression"].as_str().unwrap_or_default();
        if method == "Target.closeTarget" {
            close = Some(call["id"].clone());
            send(&mut socket, binding_called(1)).await;
            continue;
        }
        if method == "Runtime.evaluate"
            && call["params"]["contextId"] == 1
            && !expression.starts_with("__maestroReply(")
        {
            send(&mut socket, detached.clone()).await;
            let close = close.take().expect("the target close is held");
            send(&mut socket, json!({"id":close,"result":{}})).await;
            continue;
        }
        let response = if expression.starts_with("__maestroReply(") {
            json!({"id":call["id"],"error":{"code":-32001,"message":"Session with given id not found."}})
        } else if method == "Runtime.enable" {
            send(&mut socket, context_created("session", 1, "root")).await;
            json!({"id":call["id"],"result":{}})
        } else {
            let result = if method == "Runtime.evaluate" {
                json!({"result":{"value":null}})
            } else {
                shared_reply(&call).unwrap_or_else(|| panic!("unexpected command {method}"))
            };
            json!({"id":call["id"],"result":result})
        };
        send(&mut socket, response).await;
        if method == "Target.disposeBrowserContext" {
            break;
        }
    }
}
