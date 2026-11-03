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
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let socket = tokio_tungstenite::accept_async(stream).await.unwrap();
                scripted(socket, outcome).await
            });
            let mut dispatcher = Dispatcher::connect(&format!("ws://{address}"))
                .await
                .unwrap();
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
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let peer = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let socket = tokio_tungstenite::accept_async(stream).await.unwrap();
        attached_iframe(socket, waiting).await;
    });
    let mut dispatcher = Dispatcher::connect(&format!("ws://{address}"))
        .await
        .unwrap();
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

/// Emit a native iframe attachment, then its default world when Runtime is enabled.
async fn attached_iframe(mut socket: WebSocketStream<TcpStream>, waiting: bool) {
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
                socket.send(Message::text(json!({"method":"Target.attachedToTarget","sessionId":"session",
                    "params":{"sessionId":"child-session","waitingForDebugger":waiting,
                        "targetInfo":{"targetId":"child","type":"iframe","browserContextId":"context",
                            "title":"","url":"about:srcdoc","attached":true,"canAccessOpener":false}}}).to_string())).await.unwrap();
                json!({"result":{"value":true}})
            }
            "Runtime.enable" => {
                assert_eq!(call["sessionId"], "child-session");
                socket.send(Message::text(json!({"method":"Runtime.executionContextCreated","sessionId":"child-session",
                    "params":{"context":{"id":2,"origin":"null","name":"","uniqueId":"child-world","auxData":{"isDefault":true,"frameId":"child"}}}}).to_string())).await.unwrap();
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
