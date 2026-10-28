//! The shipped game uses the native session's real sequential sibling tool runner.
use maestro_app::{
    AgentEvent, AgentSession, AgentSessionEvent, ExtensionBindings, ExtensionUIContext, NoTools,
    PromptOptions, ReadonlySessionManager,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};
use tokio::sync::oneshot;
mod renderers;
mod ui;

/// Builds the shipped component in an isolated session without ambient credentials.
#[test]
fn tic_tac_toe_command_and_sequential_siblings_use_the_native_runner() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, async {
        let root = super::support::Root::new();
        let session = super::support::session::component_session(
            &root,
            local.clone(),
            std::env::var("MAESTRO_TIC_TAC_TOE_COMPONENT").expect("nextest author-component setup"),
            Some(NoTools::Builtin),
            super::support::widgets(),
        )
        .await;
        exercise(session).await;
    });
}

/// Plays a human mark, then five accepted sibling calls interleaved with three errors.
async fn exercise(session: Rc<AgentSession>) {
    assert_eq!(
        session.get_active_tool_names(),
        ["tic_tac_toe", "tic_tac_toe_see_board"]
    );
    let runner = session.extension_runner();
    let command = runner
        .get_command("tic-tac-toe")
        .expect("the shipped command");
    (command.command.handler)(String::new(), runner.create_command_context())
        .await
        .unwrap();
    let (base, worker) = provider();
    configure(&session, &base).await;
    let (ui, ready) = ui::Ui::new();
    session
        .bind_extensions(ExtensionBindings {
            ui_context: Some(ExtensionUIContext::new(ui.clone()).into()),
            ..ExtensionBindings::default()
        })
        .await
        .unwrap();
    let (unsubscribe, end) = finished(&session);
    let context = runner.create_command_context();
    let opened = tokio::task::spawn_local(async move {
        (command.command.handler)(String::new(), context)
            .await
            .unwrap();
    });
    ready.await.unwrap();
    assert!(ui.render(80).join("\n").contains("Tic-Tac-Toe"));
    assert!(ui.render(0).iter().all(String::is_empty));
    for key in ["\x1b[A", "\x1b[D", "\x1b[C", "\x1b[B", "\r"] {
        ui.input(key);
    }
    end.await.unwrap();
    let details = snapshots(&session);
    renderers::exercise(&session, &details);
    session
        .prompt("Restate the turn protocol", PromptOptions::default())
        .await
        .unwrap();
    ui.input("\x1b");
    opened.await.unwrap();
    reopen(&session).await;
    unsubscribe();
    let requests = worker.join().unwrap();
    assert!(
        requests[2]["messages"][0]["content"]
            .to_string()
            .contains("Tic-tac-toe: play O"),
        "{}",
        requests[2]
    );
    session.dispose();
}

/// Reopens the retained branch after runner errors before and after valid snapshots.
async fn reopen(session: &Rc<AgentSession>) {
    session.reload().await.unwrap();
    let runner = session.extension_runner();
    let command = runner.get_command("tic-tac-toe").unwrap();
    let (reopened_ui, ready) = ui::Ui::new();
    session
        .bind_extensions(ExtensionBindings {
            ui_context: Some(ExtensionUIContext::new(reopened_ui.clone()).into()),
            ..ExtensionBindings::default()
        })
        .await
        .unwrap();
    let context = runner.create_command_context();
    let mut reopened = tokio::task::spawn_local(async move {
        (command.command.handler)(String::new(), context)
            .await
            .unwrap();
    });
    tokio::select! {
        ready = ready => ready.unwrap(),
        result = &mut reopened => panic!("command closed before opening: {result:?}"),
    }
    let board = maestro_app::strip_ansi(&reopened_ui.render(80).join("\n")).unwrap();
    assert!(board.contains('╳') && board.contains('◯'), "{board}");
    reopened_ui.input("\x1b");
    reopened.await.unwrap();
}

/// Verifies each persisted sibling result, not just final tool metadata.
fn snapshots(session: &AgentSession) -> Value {
    let entries = serde_json::to_value(session.session_manager().borrow().get_entries()).unwrap();
    let results: Vec<_> = entries
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["message"]["role"] == "toolResult")
        .collect();
    assert_eq!(results.len(), 8);
    let errors: Vec<_> = results
        .iter()
        .filter(|result| result["message"]["isError"] == true)
        .collect();
    assert_eq!(errors.len(), 3);
    for error in errors {
        assert_eq!(error["message"]["details"], json!({}));
    }
    let results: Vec<_> = results
        .into_iter()
        .filter(|result| result["message"]["isError"] == false)
        .collect();
    for (result, (row, col)) in results.iter().zip([(1, 0), (1, 1), (2, 1), (2, 2), (0, 0)]) {
        assert_eq!(
            result["message"]["details"]["agentCursorRow"], row,
            "{result}"
        );
        assert_eq!(result["message"]["details"]["agentCursorCol"], col);
        assert_eq!(result["message"]["isError"], false);
    }
    let details = results[4]["message"]["details"].clone();
    assert_eq!(
        details["board"],
        json!([[" ", " ", " "], [" ", "X", " "], [" ", " ", "O"]])
    );
    assert!(details.get("userCursorRow").is_none());
    assert!(details.get("userCursorCol").is_none());
    assert_eq!(
        session
            .session_manager()
            .borrow()
            .get_session_name()
            .unwrap(),
        Some("Tic-Tac-Toe")
    );
    details
}

/// Registers a single loopback model through the delivered catalog interface.
async fn configure(session: &AgentSession, base: &str) {
    let config = serde_json::from_value(json!({"baseUrl":base,"apiKey":"controlled-game-key","api":"openai-completions","models":[{"id":"game","name":"controlled","reasoning":false,"input":["text"],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":128_000,"maxTokens":1000}]})).unwrap();
    session
        .model_registry()
        .register_provider("game-controlled", Arc::new(RwLock::new(config)))
        .unwrap();
    session
        .set_model(
            session
                .model_registry()
                .find("game-controlled", "game")
                .unwrap(),
        )
        .await
        .unwrap();
}

/// Serves exactly one sibling batch, its final answer, and a prompt-guidance probe.
fn provider() -> (String, std::thread::JoinHandle<Vec<Value>>) {
    use std::{io::Write, net::TcpListener};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let worker = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..3 {
            let (mut stream, _) = listener.accept().unwrap();
            requests.push(super::support::provider::request(&mut stream));
            let delta = if index == 0 {
                let calls: Vec<_> = ["move_down","move_right","play","move_down","move_right","play","play","invalid"].iter().enumerate().map(|(index,action)| json!({"index":index,"id":format!("game-{index}"),"type":"function","function":{"name":"tic_tac_toe","arguments":json!({"action":action}).to_string()}})).collect();
                json!({"tool_calls":calls})
            } else {
                json!({"content":"Your turn, X!"})
            };
            let chunk = json!({"id":"game","choices":[{"index":0,"delta":delta,"finish_reason":if index == 0 { "tool_calls" } else { "stop" }}]});
            let events = format!("data: {chunk}\n\ndata: [DONE]\n\n");
            write!(stream,"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{events}",events.len()).unwrap();
        }
        requests
    });
    (base, worker)
}

/// Observes run completion through the session, never through a timer.
fn finished(session: &AgentSession) -> (impl Fn(), oneshot::Receiver<()>) {
    let (finished, end) = oneshot::channel();
    let finished = RefCell::new(Some(finished));
    let unsubscribe = session.subscribe(Rc::new(move |event| {
        if let AgentSessionEvent::Agent(event) = event
            && matches!(&*event.read().unwrap(), AgentEvent::AgentEnd { .. })
            && let Some(finished) = finished.borrow_mut().take()
        {
            let _ = finished.send(());
        }
    }));
    (unsubscribe, end)
}

/// Ordinary runner errors render their text without snapshot details for either tool.
#[test]
fn tic_tac_toe_error_rows_do_not_require_board_snapshots() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, async {
        let root = super::support::Root::new();
        let session = super::support::session::component_session(
            &root,
            local.clone(),
            std::env::var("MAESTRO_TIC_TAC_TOE_COMPONENT").expect("nextest author-component setup"),
            Some(NoTools::Builtin),
            super::support::widgets(),
        )
        .await;
        renderers::errors(&session);
        session.dispose();
    });
}
