//! The interactive flow: stream a session's output, read prompts, and stop or abort on
//! request.
use maestro_app::{RpcClient, RpcClientError, compact_json, is_whitespace_scalar};
use serde_json::Value;
use std::{cell::RefCell, io::Write, rc::Rc};
use tokio::sync::mpsc::UnboundedReceiver;

/// Where the flow writes.
pub type Output = Rc<RefCell<dyn Write>>;

/// How many characters of a tool result the flow shows.
const PREVIEW_CHARS: usize = 200;

/// Runs the flow until an `exit` line, which stops the client, or an interrupt while no turn
/// runs, which asks the child to terminate without waiting for its end. Lines that arrive during a turn are ignored; an interrupt
/// during a turn sends an abort without waiting for it.
///
/// # Errors
/// Returns the client's failure to start, to read the state or to run a turn, and output
/// failures.
pub async fn run(
    client: Rc<RpcClient>,
    mut input: UnboundedReceiver<String>,
    mut interrupts: UnboundedReceiver<()>,
    output: Output,
) -> Result<(), RpcClientError> {
    let events = Rc::clone(&output);
    let _render = client.on_event(Rc::new(move |event: &Value| {
        Ok(render_event(event, &mut *events.borrow_mut())?)
    }));
    client.start().await?;
    write_state(&client, &output).await?;
    write!(output.borrow_mut(), "You: ")?;
    output.borrow_mut().flush()?;
    let mut turn = None;
    loop {
        tokio::select! {
            biased;
            Some(()) = interrupts.recv() => {
                if turn.is_none() {
                    tokio::task::spawn_local(async move { client.stop().await });
                    // One yield runs the stop and the child's driver, which asks the child to
                    // terminate before the caller tears the task set down.
                    tokio::task::yield_now().await;
                    return Ok(());
                }
                writeln!(output.borrow_mut(), "\n[Aborting...]")?;
                let client = Rc::clone(&client);
                tokio::task::spawn_local(async move { client.abort().await });
            }
            Some(line) = input.recv() => {
                if turn.is_some() {
                    continue;
                }
                if line.trim_matches(is_whitespace_scalar) == "exit" {
                    return client.stop().await;
                }
                let client = Rc::clone(&client);
                turn = Some(Box::pin(async move { client.prompt_and_wait(&line, None, None).await }));
            }
            Some(outcome) = finish(&mut turn) => {
                turn = None;
                outcome?;
                write!(output.borrow_mut(), "\n\nYou: ")?;
                output.borrow_mut().flush()?;
            }
            else => return Ok(()),
        }
    }
}

/// The outcome of the running turn; `None` at once without one.
async fn finish<F: std::future::Future + Unpin>(turn: &mut Option<F>) -> Option<F::Output> {
    match turn {
        Some(turn) => Some(turn.await),
        None => None,
    }
}

/// Writes the selected model and the thinking level, then the greeting.
async fn write_state(client: &RpcClient, output: &Output) -> Result<(), RpcClientError> {
    let state = client.get_state().await?;
    let (provider, id) = state.model.as_ref().map_or_else(
        || ("undefined".to_owned(), "undefined".to_owned()),
        |model| {
            let model = model
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (model.provider.clone(), model.id.clone())
        },
    );
    let level = serde_json::to_value(state.thinking_level)?;
    let mut output = output.borrow_mut();
    writeln!(output, "Model: {provider}/{id}")?;
    writeln!(output, "Thinking: {}\n", level.as_str().unwrap_or("off"))?;
    writeln!(output, "Interactive RPC example. Type 'exit' to quit.\n")?;
    Ok(())
}

/// Writes a text or thinking delta, a tool's name, or the start of a tool's result.
fn render_event(event: &Value, output: &mut dyn Write) -> std::io::Result<()> {
    match event["type"].as_str() {
        Some("message_update") => {
            let update = &event["assistantMessageEvent"];
            if matches!(
                update["type"].as_str(),
                Some("text_delta" | "thinking_delta")
            ) && let Some(delta) = update["delta"].as_str()
            {
                output.write_all(delta.as_bytes())?;
            }
            output.flush()
        }
        Some("tool_execution_start") => {
            let name = event["toolName"].as_str().unwrap_or("undefined");
            writeln!(output, "\n[Tool: {name}]")
        }
        Some("tool_execution_end") => {
            writeln!(output, "[Result: {}...]\n", preview(&event["result"])?)
        }
        _ => Ok(()),
    }
}

/// The first characters of `result` written as JSON.
fn preview(result: &Value) -> Result<String, serde_json::Error> {
    Ok(compact_json(result)?.chars().take(PREVIEW_CHARS).collect())
}

/// Writes a failure of the flow to `stderr`.
pub fn report(result: Result<(), RpcClientError>, stderr: &mut dyn Write) {
    if let Err(error) = result {
        let _ = writeln!(stderr, "{error}");
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use crate::rpc_peer::{Peer, events, next_of};
    use crate::support::session_harness::on_task_set;
    use serde_json::json;
    use tokio::sync::mpsc;

    /// Answers the flow's commands and records each in its diagnostic text. A prompt of
    /// `hold` runs until a steering message or an abort arrives.
    const SESSION: &str = r"
state = json.loads(os.environ['PEER_STATE'])
turn = [{'type': 'message_update', 'assistantMessageEvent': {'type': 'text_delta', 'delta': 'Hi there'}},
        {'type': 'message_update', 'assistantMessageEvent': {'type': 'toolcall_delta', 'delta': 'skipped'}},
        {'type': 'message_update', 'assistantMessageEvent': {'type': 'thinking_delta', 'delta': ' (thinking)'}},
        {'type': 'tool_execution_start', 'toolName': 'read'},
        {'type': 'tool_execution_end', 'result': {'content': [{'type': 'text', 'text': 'ok'}]}},
        {'type': 'turn_end'}, {'type': 'agent_end', 'messages': []}]
signal.signal(signal.SIGTERM, lambda *_: (os.write(2, b'terminated\n'), os._exit(0)))
for line in commands():
    command = json.loads(line)
    os.write(2, (json.dumps(command) + '\n').encode())
    kind = command['type']
    respond(command, state if kind == 'get_state' else MISSING)
    if kind == 'prompt' and command['message'] == 'hold': emit({'type': 'agent_start'})
    elif kind == 'prompt': [emit(record) for record in turn]
    elif kind in ('steer', 'abort'): emit({'type': 'agent_end', 'messages': []})
";

    /// A session state with `model`.
    fn state(model: Option<Value>) -> Value {
        let mut state = json!({"thinkingLevel": "high", "isStreaming": false, "isCompacting": false, "steeringMode": "all", "followUpMode": "all", "sessionId": "s", "autoCompactionEnabled": true, "messageCount": 0, "pendingMessageCount": 0});
        if let Some(model) = model {
            state["model"] = model;
        }
        state
    }

    /// The selected model of the illustration.
    fn model() -> Value {
        json!({"provider": "anthropic", "id": "claude-sonnet-4-20250514", "name": "Sonnet", "api": "anthropic-messages", "baseUrl": "https://fixture.invalid", "reasoning": true, "input": ["text"], "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0}, "contextWindow": 1, "maxTokens": 1})
    }

    /// Output that announces each `You: ` prompt written to it.
    struct Recorder {
        /// Everything written.
        text: Vec<u8>,
        /// Receives one signal per prompt.
        prompts: mpsc::UnboundedSender<()>,
    }

    impl Write for Recorder {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.text.extend_from_slice(bytes);
            if self.text.ends_with(b"You: ") {
                let _ = self.prompts.send(());
            }
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// One flow under test, with its inputs and its recorded output.
    struct Flow {
        /// The client the flow drives.
        client: Rc<RpcClient>,
        /// Feeds input lines.
        lines: mpsc::UnboundedSender<String>,
        /// Feeds interrupts.
        interrupt: mpsc::UnboundedSender<()>,
        /// Signals each written prompt.
        prompts: mpsc::UnboundedReceiver<()>,
        /// The recorded output.
        output: Rc<RefCell<Recorder>>,
        /// The running flow.
        run: tokio::task::JoinHandle<Result<(), RpcClientError>>,
    }

    impl Flow {
        /// Starts the flow on a client of `peer` whose session state is `state`.
        fn start(peer: &Peer, state: &Value) -> Self {
            let mut options = peer.options(SESSION);
            options.env.insert("PEER_STATE".into(), state.to_string());
            let client = Rc::new(RpcClient::new(options));
            let (lines, input) = mpsc::unbounded_channel();
            let (interrupt, interrupts) = mpsc::unbounded_channel();
            let (signal, prompts) = mpsc::unbounded_channel();
            let output = Rc::new(RefCell::new(Recorder {
                text: Vec::new(),
                prompts: signal,
            }));
            let shared: Output = output.clone();
            let run = tokio::task::spawn_local(run(Rc::clone(&client), input, interrupts, shared));
            Self {
                client,
                lines,
                interrupt,
                prompts,
                output,
                run,
            }
        }

        /// Sends `line` once the next prompt is written.
        async fn answer(&mut self, line: &str) {
            self.prompts.recv().await.unwrap();
            self.lines.send(line.to_owned()).unwrap();
        }

        /// The flow's outcome once it ends.
        async fn finished(&mut self) -> Result<(), RpcClientError> {
            (&mut self.run).await.unwrap()
        }

        /// The recorded output.
        fn text(&self) -> String {
            String::from_utf8(self.output.borrow().text.clone()).unwrap()
        }

        /// The commands the child received, in order.
        fn commands(&self) -> Vec<Value> {
            let stderr = self.client.get_stderr();
            stderr
                .lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        }
    }

    #[test]
    fn illustration_streams_state_text_thinking_and_tool_output() {
        let peer = Peer::new();
        on_task_set(|_| async {
            for (model, selected) in [
                (Some(model()), "anthropic/claude-sonnet-4-20250514"),
                (None, "undefined/undefined"),
            ] {
                let mut flow = Flow::start(&peer, &state(model));
                flow.answer("hello").await;
                flow.answer("exit").await;
                flow.finished().await.unwrap();
                let header = format!(
                    "Model: {selected}\nThinking: high\n\nInteractive RPC example. Type 'exit' to quit.\n\n"
                );
                let result = r#"{"content":[{"type":"text","text":"ok"}]}"#;
                let turn = format!("Hi there (thinking)\n[Tool: read]\n[Result: {result}...]\n\n");
                assert_eq!(flow.text(), format!("{header}You: {turn}\n\nYou: "));
            }
        });
    }

    #[test]
    fn illustration_keeps_original_lines_and_exact_exit_whitespace() {
        let peer = Peer::new();
        on_task_set(|_| async {
            let mut flow = Flow::start(&peer, &state(None));
            flow.answer("  hello  ").await;
            flow.answer("\u{85}exit").await;
            flow.answer("\u{feff} exit\t").await;
            flow.finished().await.unwrap();
            let prompts: Vec<_> = flow
                .commands()
                .into_iter()
                .filter(|command| command["type"] == "prompt")
                .map(|command| command["message"].clone())
                .collect();
            assert_eq!(prompts, [json!("  hello  "), json!("\u{85}exit")]);
            assert!(matches!(
                flow.client.abort().await,
                Err(RpcClientError::NotStarted)
            ));
        });
    }

    #[test]
    fn illustration_suppresses_input_while_a_turn_is_waiting() {
        let peer = Peer::new();
        on_task_set(|_| async {
            let mut flow = Flow::start(&peer, &state(None));
            let mut events = events(&flow.client);
            flow.answer("hold").await;
            next_of(&mut events, "agent_start").await;
            flow.lines.send("exit".into()).unwrap();
            flow.lines.send("ignored".into()).unwrap();
            flow.client.steer("finish", None).await.unwrap();
            flow.answer("exit").await;
            flow.finished().await.unwrap();
            let kinds: Vec<_> = flow
                .commands()
                .into_iter()
                .map(|command| command["type"].clone())
                .collect();
            assert_eq!(kinds, [json!("get_state"), json!("prompt"), json!("steer")]);
            assert!(flow.text().ends_with("You: \n\nYou: "));
        });
    }

    #[test]
    fn illustration_interrupts_follow_the_waiting_state() {
        let peer = Peer::new();
        on_task_set(|_| async {
            let mut flow = Flow::start(&peer, &state(None));
            let mut events = events(&flow.client);
            flow.answer("hold").await;
            next_of(&mut events, "agent_start").await;
            flow.interrupt.send(()).unwrap();
            flow.prompts.recv().await.unwrap();
            assert!(flow.text().ends_with("You: \n[Aborting...]\n\n\nYou: "));
            flow.interrupt.send(()).unwrap();
            flow.finished().await.unwrap();
            let kinds: Vec<_> = flow
                .commands()
                .into_iter()
                .map(|command| command["type"].clone())
                .collect();
            assert_eq!(kinds, [json!("get_state"), json!("prompt"), json!("abort")]);
            while !flow.client.get_stderr().contains("terminated") {
                tokio::task::yield_now().await;
            }
        });
    }

    #[test]
    fn illustration_idle_interrupt_requests_termination_before_teardown() {
        let peer = Peer::new();
        let marker = peer.scratch.join("marker");
        let script = r"
mark = lambda text: os.write(os.open(os.environ['PEER_MARKER'], os.O_WRONLY | os.O_CREAT), text)
signal.signal(signal.SIGTERM, lambda *_: (mark(b'terminated'), os._exit(0)))
for line in commands(): respond(json.loads(line), json.loads(os.environ['PEER_STATE']))
mark(b'input closed')
";
        let mut options = peer.options(script);
        options.env.insert("PEER_MARKER".into(), marker.clone());
        options
            .env
            .insert("PEER_STATE".into(), state(None).to_string());
        let (signal, mut prompts) = mpsc::unbounded_channel();
        let output: Output = Rc::new(RefCell::new(Recorder {
            text: Vec::new(),
            prompts: signal,
        }));
        let (_lines, input) = mpsc::unbounded_channel();
        let (interrupt, interrupts) = mpsc::unbounded_channel();
        let runtime = crate::support::io_runtime();
        tokio::task::LocalSet::new().block_on(&runtime, async {
            let interrupting = async {
                prompts.recv().await.unwrap();
                interrupt.send(()).unwrap();
            };
            let client = Rc::new(RpcClient::new(options));
            let (result, ()) = tokio::join!(run(client, input, interrupts, output), interrupting);
            result.unwrap();
        });
        // The task set is gone, as it is once the example's entry returns.
        let mut ended = String::new();
        while ended.is_empty() {
            std::thread::yield_now();
            ended = std::fs::read_to_string(&marker).unwrap_or_default();
        }
        assert_eq!(ended, "terminated");
    }

    #[test]
    fn illustration_preview_keeps_the_boundary_scalar() {
        let quoted = |text: String| preview(&json!(text)).unwrap();
        assert_eq!(preview(&Value::Null).unwrap(), "null");
        for length in [199, 200] {
            let text = "a".repeat(length - 2);
            assert_eq!(quoted(text.clone()), format!("\"{text}\""));
        }
        assert_eq!(quoted("a".repeat(199)), format!("\"{}", "a".repeat(199)));
        let boundary = format!("{}\u{1f642}tail", "a".repeat(198));
        assert_eq!(quoted(boundary), format!("\"{}\u{1f642}", "a".repeat(198)));
    }

    #[test]
    fn illustration_reports_top_level_error_without_extra_diagnostics() {
        let peer = Peer::new();
        on_task_set(|_| async {
            let mut options = peer.options(SESSION);
            options.cli_path = Some(peer.scratch.join("missing"));
            let (_lines, input) = mpsc::unbounded_channel();
            let (_interrupt, interrupts) = mpsc::unbounded_channel();
            let (signal, _prompts) = mpsc::unbounded_channel();
            let output = Rc::new(RefCell::new(Recorder {
                text: Vec::new(),
                prompts: signal,
            }));
            let shared: Output = output.clone();
            let result = run(Rc::new(RpcClient::new(options)), input, interrupts, shared).await;
            assert!(output.borrow().text.is_empty());
            let mut stderr = Vec::new();
            report(result, &mut stderr);
            assert_eq!(
                String::from_utf8(stderr).unwrap(),
                "No such file or directory (os error 2)\n"
            );
            let mut stderr = Vec::new();
            report(Ok(()), &mut stderr);
            assert!(stderr.is_empty());
        });
    }
}
