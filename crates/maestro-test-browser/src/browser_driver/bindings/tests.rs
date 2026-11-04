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
    binding_called_from("session", 1, id)
}

/// Build inspection call `id` as renderer `session` emits it from execution context `context`.
fn binding_called_from(session: &str, context: i64, id: i64) -> Value {
    let payload = json!({"id":id,"name":"inspectSandbox","args":["probe:\"雪]","state"]});
    json!({
        "method":"Runtime.bindingCalled","sessionId":session,
        "params":{
            "name":"__maestroDriver","executionContextId":context,"payload":payload.to_string()
        }
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
        | "Emulation.setTouchEmulationEnabled"
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

/// What interrupts an iframe's setup while the root evaluation is held.
#[derive(Clone, Copy)]
enum Interruption {
    /// The frame is removed, and the root page's host call arrives before the frame's command ends.
    RemovedWithRootCall,
    /// The frame calls the host itself, then is removed.
    RemovedWithOwnCall,
    /// The root page throws an unexpected exception.
    RootException,
}

/// Run one interrupted setup behind a held root evaluation.
///
/// Returns the awaited command's outcome and every reply the driver evaluated in the root world.
async fn interrupted(interruption: Interruption) -> (io::Result<Value>, Vec<String>) {
    let (mut dispatcher, peer) =
        connect_peer(move |socket| interrupted_setup(socket, interruption)).await;
    let mut lines = Vec::new();
    let expected = inputs::inventory(INVENTORY).unwrap().expected_error;
    let mut page = Page::open(&mut dispatcher, "removed", &mut lines, &expected)
        .await
        .unwrap();
    page.initialize().await.unwrap();
    let outcome = page.evaluate(HOLD).await;
    page.close().await.unwrap();
    (outcome, peer.await.unwrap())
}

/// An iframe removed while its setup commands are in flight settles its own host call and leaves
/// the awaited command alone.
#[test]
fn iframe_removed_during_setup_settles_its_host_call() {
    block_on(async {
        let (outcome, replies) = interrupted(Interruption::RemovedWithRootCall).await;
        outcome.unwrap();
        assert_eq!(
            replies,
            [format!(
                "__maestroReply(1,null,{})",
                json!("mounted frame not found probe:\"雪]")
            )]
        );
    });
}

/// A host call queued by the frame whose setup failed has no caller left, so it is ignored.
#[test]
fn host_call_queued_by_a_removed_frame_is_ignored() {
    block_on(async {
        let (outcome, replies) = interrupted(Interruption::RemovedWithOwnCall).await;
        outcome.unwrap();
        assert_eq!(replies, Vec::<String>::new());
    });
}

/// A page-level failure raised while an iframe is being set up still fails the awaited command.
#[test]
fn page_exception_during_iframe_setup_fails_the_command() {
    block_on(async {
        let (outcome, _) = interrupted(Interruption::RootException).await;
        let error = outcome.unwrap_err().to_string();
        assert!(error.contains("Error: page failed"), "{error}");
    });
}

/// Every key name the qualification exports press resolves through the driver's key table.
#[test]
fn press_resolves_every_key_name_the_qualification_exports_press() {
    for chord in [
        "Space",
        "Enter",
        "Escape",
        "Tab",
        "Backspace",
        "ArrowUp",
        "ArrowDown",
        "Shift+Enter",
        "Shift+Tab",
    ] {
        let key = chord.split('+').next_back().unwrap();
        assert!(key_definition(key).is_some(), "{chord} does not resolve");
    }
    assert!(
        key_definition("Mute").is_none(),
        "unmapped names stay unknown"
    );
}

/// Holding Shift still types the key's text, while any other held modifier suppresses it.
#[test]
fn press_types_key_text_under_shift_only() {
    for (chord, modifiers, text) in [
        ("Enter", 0, Some("\r")),
        ("Shift+Enter", 8, Some("\r")),
        ("Control+Enter", 2, None),
        ("Shift+Alt+Enter", 9, None),
        ("Shift+Tab", 8, None),
    ] {
        let [down, up] = key_events(chord).unwrap();
        assert_eq!(down["type"], "keyDown", "{chord}");
        assert_eq!(down["modifiers"], modifiers, "{chord}");
        assert_eq!(down.get("text").and_then(Value::as_str), text, "{chord}");
        assert_eq!(
            down.get("unmodifiedText").and_then(Value::as_str),
            text,
            "{chord}"
        );
        assert_eq!(up["type"], "keyUp", "{chord}");
        assert_eq!(up.get("text"), None, "{chord}");
    }
}

/// Navigation readiness runs in the announced committed world, not in a context-free root
/// evaluation, whichever side of the navigate reply announces that world.
#[test]
fn navigation_readiness_waits_for_the_committed_default_context() {
    block_on(async {
        for commit_first in [true, false] {
            let (mut dispatcher, peer) =
                connect_peer(move |socket| navigating_peer(socket, commit_first)).await;
            let mut lines = Vec::new();
            let expected = inputs::inventory(INVENTORY).unwrap().expected_error;
            let mut page = Page::open(&mut dispatcher, "navigate", &mut lines, &expected)
                .await
                .unwrap();
            page.navigate("fixture").await.unwrap();
            assert_eq!(
                page.root_context().unwrap().id,
                2,
                "commit_first={commit_first}"
            );
            page.close().await.unwrap();
            peer.await.unwrap();
        }
    });
}

/// A scripted peer that answers the navigation while the commit is announced on either side of
/// that reply, and rejects a context-free root evaluation until the committed document announces
/// its own default world.
async fn navigating_peer(mut socket: WebSocketStream<TcpStream>, commit_first: bool) -> Option<()> {
    while let Some(call) = receive(&mut socket).await {
        let method = call["method"].as_str().unwrap();
        if method == "Page.navigate" {
            commit_navigation(&mut socket, &call, commit_first).await;
            continue;
        }
        if method == "Runtime.evaluate" && call["params"]["contextId"].is_null() {
            send(
                &mut socket,
                json!({"id":call["id"],"error":{"code":-32000,
                    "message":"Cannot find default execution context"}}),
            )
            .await;
            continue;
        }
        if method == "Runtime.evaluate" && call["params"]["contextId"] == 2 {
            let expression = call["params"]["expression"].as_str().unwrap_or_default();
            assert!(expression.starts_with("new Promise"), "{expression}");
            send(
                &mut socket,
                json!({"id":call["id"],"result":{"result":{"value":null}}}),
            )
            .await;
            continue;
        }
        let result = shared_reply(&call).unwrap_or_else(|| panic!("unexpected command {method}"));
        let leave = method == "Target.disposeBrowserContext";
        send(&mut socket, json!({"id":call["id"],"result":result})).await;
        if leave {
            return Some(());
        }
    }
    None
}

/// Answer the navigate with its frame, announcing the committed document on the side of the reply
/// `commit_first` chooses.
async fn commit_navigation(
    socket: &mut WebSocketStream<TcpStream>,
    call: &Value,
    commit_first: bool,
) {
    let reply = json!({"id":call["id"],"result":{"frameId":"committed","loaderId":"loader"}});
    let destroyed = json!({"method":"Runtime.executionContextDestroyed","sessionId":"session",
            "params":{"executionContextId":1,"executionContextUniqueId":"root-world"}});
    let navigated = json!({"method":"Page.frameNavigated","sessionId":"session",
            "params":{"type":"Navigation","frame":{"id":"committed","loaderId":"loader","url":"fixture",
                "domainAndRegistry":"","securityOrigin":"null","mimeType":"text/html",
                "secureContextType":"InsecureScheme","crossOriginIsolatedContextType":"NotIsolated",
                "gatedAPIFeatures":[]}}});
    let committed = context_created("session", 2, "committed");
    if commit_first {
        send(socket, destroyed).await;
        send(socket, navigated).await;
        send(socket, committed).await;
        send(socket, reply).await;
    } else {
        send(socket, reply).await;
        send(socket, destroyed).await;
        send(socket, navigated).await;
        send(socket, committed).await;
    }
}

/// Page creation leaves the desktop pointer to the launch environment and enables touch emulation
/// only for the export that asks for touch input.
#[test]
fn page_creation_enables_touch_emulation_only_for_its_touch_opted_export() {
    block_on(async {
        for (name, touch) in [
            ("desktop", false),
            (
                "tile_dom_remove_does_not_open_and_uses_current_callback",
                true,
            ),
        ] {
            let (mut dispatcher, peer) =
                connect_peer(move |socket| touch_pointer(socket, touch)).await;
            let mut lines = Vec::new();
            let expected = inputs::inventory(INVENTORY).unwrap().expected_error;
            let mut page = Page::open(&mut dispatcher, name, &mut lines, &expected)
                .await
                .unwrap();
            page.initialize().await.unwrap();
            page.close().await.unwrap();
            peer.await.unwrap();
        }
    });
}

/// Serve the setup sequence and refuse a page whose emulation command count differs from its
/// request.
async fn touch_pointer(mut socket: WebSocketStream<TcpStream>, touch: bool) -> Option<()> {
    let mut enabled = 0usize;
    while let Some(call) = receive(&mut socket).await {
        let method = call["method"].as_str().unwrap();
        if method == "Emulation.setDeviceMetricsOverride" {
            assert_eq!(call["params"]["mobile"], false);
            send(&mut socket, json!({"id":call["id"],"result":{}})).await;
            continue;
        }
        if method == "Emulation.setTouchEmulationEnabled" {
            assert_eq!(call["params"]["enabled"], touch);
            enabled += 1;
            send(&mut socket, json!({"id":call["id"],"result":{}})).await;
            continue;
        }
        if method == "Runtime.enable" {
            send(&mut socket, json!({"id":call["id"],"result":{}})).await;
            continue;
        }
        let result = shared_reply(&call).unwrap_or_else(|| panic!("unexpected command {method}"));
        let leave = method == "Target.disposeBrowserContext";
        send(&mut socket, json!({"id":call["id"],"result":result})).await;
        if leave {
            assert_eq!(
                enabled,
                usize::from(touch),
                "touch emulation commands on {touch}"
            );
            return Some(());
        }
    }
    None
}

/// A host call whose reply evaluation Chromium refuses with a dropped-context error, before its
/// removal announcement arrives, settles the call instead of failing the page.
#[test]
fn dropped_context_reply_refusal_does_not_fail_the_host_call() {
    block_on(async {
        let (mut dispatcher, peer) = connect_peer(dropping_context_peer).await;
        let mut lines = Vec::new();
        let expected = inputs::inventory(INVENTORY).unwrap().expected_error;
        let mut page = Page::open(&mut dispatcher, "settle", &mut lines, &expected)
            .await
            .unwrap();
        page.initialize().await.unwrap();
        let outcome = page.evaluate(HOLD).await;
        page.close().await.unwrap();
        peer.await.unwrap();
        outcome.unwrap();
        assert!(
            lines.iter().any(|line| line.contains("ready; frame")),
            "{lines:?}"
        );
    });
}

/// Serve setup, route a host call of the root world, refuse its reply the way Chromium refuses an
/// evaluation in a context its renderer has already dropped, then answer the held evaluation.
async fn dropping_context_peer(mut socket: WebSocketStream<TcpStream>) -> Option<()> {
    let mut held = None;
    while let Some(call) = receive(&mut socket).await {
        let method = call["method"].as_str().unwrap();
        let expression = call["params"]["expression"].as_str().unwrap_or_default();
        if method == "Runtime.enable" {
            send(&mut socket, enabled_context(&call)).await;
            send(&mut socket, json!({"id":call["id"],"result":{}})).await;
            continue;
        }
        if method == "Runtime.evaluate" && expression == HOLD {
            held = Some(call["id"].clone());
            send(
                &mut socket,
                json!({"method":"Runtime.bindingCalled","sessionId":"session",
                    "params":{"name":"__maestroDriver","executionContextId":1,
                        "payload":json!({"id":1,"name":"sandboxDriverPhase","args":["ready"]}).to_string()}}),
            )
            .await;
            continue;
        }
        if expression.starts_with("__maestroReply(") {
            send(
                &mut socket,
                json!({"id":call["id"],"error":{"code":-32000,
                    "message":"Cannot find context with specified id"}}),
            )
            .await;
            let held = held
                .take()
                .expect("the host call runs behind a held evaluation");
            send(
                &mut socket,
                json!({"id":held,"result":{"result":{"value":null}}}),
            )
            .await;
            continue;
        }
        let result = shared_reply(&call).unwrap_or_else(|| panic!("unexpected command {method}"));
        let leave = method == "Target.disposeBrowserContext";
        send(&mut socket, json!({"id":call["id"],"result":result})).await;
        if leave {
            return Some(());
        }
    }
    None
}

/// Attach an iframe during the held evaluation, then interrupt its setup as `interruption` says.
///
/// Returns every reply the driver evaluated in the root world.
async fn interrupted_setup(
    mut socket: WebSocketStream<TcpStream>,
    interruption: Interruption,
) -> Vec<String> {
    let [attached, _] = iframe_events(false);
    let (mut held, mut replies) = (None, Vec::new());
    while let Some(call) = receive(&mut socket).await {
        let method = call["method"].as_str().unwrap();
        let expression = call["params"]["expression"].as_str().unwrap_or_default();
        if method == "Runtime.evaluate" && expression == HOLD {
            held = Some(call["id"].clone());
            send(&mut socket, attached.clone()).await;
            continue;
        }
        if call["sessionId"] == "child-session"
            && interrupt(&mut socket, interruption, method, held.as_ref()).await
        {
            continue;
        }
        let result = match method {
            "Runtime.enable" => {
                send(&mut socket, enabled_context(&call)).await;
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
            send(&mut socket, held_result(&held)).await;
        }
        if method == "Target.disposeBrowserContext" {
            break;
        }
    }
    replies
}

/// The held evaluation's answer.
fn held_result(id: &Value) -> Value {
    json!({"id":id,"result":{"result":{"value":null}}})
}

/// Interrupt the frame's setup at the command `interruption` names; true when that command is
/// left unanswered, as Chromium leaves a command of a removed renderer. `RemovedWithOwnCall` and
/// `RootException` also answer the held root evaluation, so a driver that ignores the interruption
/// returns at once; the main loop answers it after `RemovedWithRootCall` records its reply.
async fn interrupt(
    socket: &mut WebSocketStream<TcpStream>,
    interruption: Interruption,
    method: &str,
    held: Option<&Value>,
) -> bool {
    let detached = json!({
        "method":"Target.detachedFromTarget","sessionId":"session",
        "params":{"sessionId":"child-session"}
    });
    match (interruption, method) {
        (Interruption::RemovedWithRootCall, "Page.enable") => {
            send(socket, detached).await;
            send(socket, binding_called(1)).await;
            true
        }
        (Interruption::RemovedWithOwnCall, "Target.setAutoAttach") => {
            send(socket, binding_called_from("child-session", 2, 1)).await;
            send(socket, detached).await;
            send(
                socket,
                held_result(held.expect("the root evaluation is held")),
            )
            .await;
            true
        }
        (Interruption::RootException, "Page.enable") => {
            send(socket, page_exception()).await;
            send(
                socket,
                held_result(held.expect("the root evaluation is held")),
            )
            .await;
            false
        }
        _ => false,
    }
}

/// An exception the root page throws, which is not the designated policy error.
fn page_exception() -> Value {
    json!({
        "method":"Runtime.exceptionThrown","sessionId":"session",
        "params":{"timestamp":0,"exceptionDetails":{"exceptionId":1,"text":"Uncaught",
            "lineNumber":0,"columnNumber":0,
            "exception":{"type":"object","description":"Error: page failed"}}}
    })
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
        if expression.starts_with("__maestroReply(") {
            let refused = json!({"code":-32001,"message":"Session with given id not found."});
            send(&mut socket, json!({"id":call["id"],"error":refused})).await;
            continue;
        }
        let result = match method {
            "Runtime.enable" => {
                send(&mut socket, context_created("session", 1, "root")).await;
                json!({})
            }
            "Runtime.evaluate" => json!({"result":{"value":null}}),
            _ => shared_reply(&call).unwrap_or_else(|| panic!("unexpected command {method}")),
        };
        send(&mut socket, json!({"id":call["id"],"result":result})).await;
        if method == "Target.disposeBrowserContext" {
            break;
        }
    }
}

/// A sandboxed frame calls the host once its binding script is installed, whether Chromium
/// attached it before its document committed (loaded from a URL) or after (`srcdoc`).
#[test]
fn host_calls_from_frames_attached_before_and_after_commit_are_answered() {
    block_on(async {
        for committed in [false, true] {
            let (mut dispatcher, peer) =
                connect_peer(move |socket| committing_frame(socket, committed)).await;
            let mut lines = Vec::new();
            let expected = inputs::inventory(INVENTORY).unwrap().expected_error;
            let mut page = Page::open(&mut dispatcher, "commit", &mut lines, &expected)
                .await
                .unwrap();
            page.initialize().await.unwrap();
            let outcome = page.evaluate(HOLD).await;
            page.close().await.unwrap();
            let replies = peer.await.unwrap();
            outcome.unwrap_or_else(|error| panic!("committed={committed}: {error}"));
            assert_eq!(replies, ["__maestroReply(1,null,null)"], "{committed}");
        }
    });
}

/// Chromium's frame for the page, as the scripted peer plays it.
///
/// A frame attached before its commit has no default context, so it rejects a context-free
/// evaluation. A script runs where Chromium would run it: at once in a committed document when
/// `runImmediately` is set, and at every later commit.
struct Frame {
    /// Whether the document has committed when the page attaches.
    committed: bool,
    /// Whether the binding script is installed in the frame.
    installed: bool,
    /// Every reply the driver evaluated in the frame.
    replies: Vec<String>,
}

impl Frame {
    /// The attachment record, sent while the root evaluation is held.
    fn attached(&self) -> Value {
        let url = if self.committed {
            "about:srcdoc"
        } else {
            "data:text/html,ready"
        };
        json!({
            "method":"Target.attachedToTarget","sessionId":"session",
            "params":{"sessionId":"child-session","waitingForDebugger":!self.committed,
                "targetInfo":{"targetId":"child","type":"iframe","browserContextId":"context",
                    "title":"","url":url,"attached":true,"canAccessOpener":false}}
        })
    }

    /// Answer one command of the page or its frame; `Err` is the rejection to send instead.
    async fn answer(
        &mut self,
        socket: &mut WebSocketStream<TcpStream>,
        call: &Value,
    ) -> Result<Value, Value> {
        let method = call["method"].as_str().unwrap();
        let expression = call["params"]["expression"].as_str().unwrap_or_default();
        let child = call["sessionId"] == "child-session";
        Ok(match method {
            "Runtime.enable" => {
                if self.committed || !child {
                    send(socket, enabled_context(call)).await;
                }
                json!({})
            }
            "Page.addScriptToEvaluateOnNewDocument" if child => {
                self.installed = !self.committed || call["params"]["runImmediately"] == true;
                json!({"identifier":"1"})
            }
            "Runtime.evaluate" if child && call["params"]["contextId"].is_null() => {
                if !self.committed {
                    return Err(
                        json!({"code":-32000,"message":"Cannot find default execution context"}),
                    );
                }
                self.installed = true;
                json!({"result":{"value":null}})
            }
            "Runtime.evaluate" if expression.starts_with("__maestroReply(") => {
                self.replies.push(expression.to_owned());
                json!({"result":{"value":null}})
            }
            "Runtime.evaluate" => json!({"result":{"value":null}}),
            "Runtime.runIfWaitingForDebugger" if child => {
                self.commit(socket).await;
                json!({})
            }
            _ => shared_reply(call).unwrap_or_else(|| panic!("unexpected command {method}")),
        })
    }

    /// Commit the frame's document if it was waiting, then call the host when the script ran.
    async fn commit(&self, socket: &mut WebSocketStream<TcpStream>) {
        if !self.committed {
            let navigated = json!({
                "method":"Page.frameNavigated","sessionId":"child-session",
                "params":{"type":"Navigation","frame":{
                    "id":"child","loaderId":"loader","url":"data:text/html,ready",
                    "domainAndRegistry":"","securityOrigin":"null","mimeType":"text/html",
                    "secureContextType":"InsecureScheme",
                    "crossOriginIsolatedContextType":"NotIsolated","gatedAPIFeatures":[]
                }}
            });
            send(socket, navigated).await;
            send(socket, context_created("child-session", 2, "child")).await;
        }
        if self.installed {
            let payload = json!({"id":1,"name":"sandboxDriverPhase","args":["ready"]});
            let called = json!({
                "method":"Runtime.bindingCalled","sessionId":"child-session",
                "params":{
                    "name":"__maestroDriver","executionContextId":2,"payload":payload.to_string()
                }
            });
            send(socket, called).await;
        }
    }
}

/// Attach the frame during the held evaluation and release it once the host call is answered,
/// or at once when the script never ran.
///
/// Returns every reply the driver evaluated in the frame.
async fn committing_frame(mut socket: WebSocketStream<TcpStream>, committed: bool) -> Vec<String> {
    let mut frame = Frame {
        committed,
        installed: false,
        replies: Vec::new(),
    };
    let mut held = None;
    while let Some(call) = receive(&mut socket).await {
        let method = call["method"].as_str().unwrap();
        if method == "Runtime.evaluate" && call["params"]["expression"] == HOLD {
            held = Some(call["id"].clone());
            send(&mut socket, frame.attached()).await;
            continue;
        }
        let record = match frame.answer(&mut socket, &call).await {
            Ok(result) => json!({"id":call["id"],"result":result}),
            Err(error) => json!({"id":call["id"],"error":error}),
        };
        send(&mut socket, record).await;
        let skipped = !frame.installed && method == "Runtime.runIfWaitingForDebugger";
        if let Some(held) = held.take_if(|_| skipped || !frame.replies.is_empty()) {
            send(&mut socket, held_result(&held)).await;
        }
        if method == "Target.disposeBrowserContext" {
            break;
        }
    }
    frame.replies
}
