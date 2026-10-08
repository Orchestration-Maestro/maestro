//! Runs the example extension through the controlled adapter and checks the shared transcript.

use std::cell::Cell;
use std::rc::Rc;

use extension_example::extension;
use maestro_extensions_wasm::controlled::{ControlledHost, Flag, Session};
use maestro_extensions_wasm::{
    AbortSignal, CustomMessage, ExtensionAPI, ExtensionContext, InputEvent, InputResult,
    InputSource, ToolInvocation, prepare_arguments, run_input, run_tool,
};
use tokio::sync::oneshot;

#[path = "../../shared/scenario.rs"]
mod scenario;

/// Error type of the driver helpers.
type Failure = Box<dyn std::error::Error>;

/// Turns a missing registration into an error.
fn present<T>(value: Option<T>, what: &str) -> Result<T, Failure> {
    value.ok_or_else(|| format!("no {what} registered").into())
}

/// One-line description of an input decision.
fn describe(decision: &Result<Option<InputResult>, String>) -> String {
    match decision {
        Ok(Some(InputResult::Transform(replacement))) => format!("transform:{}", replacement.text),
        Ok(Some(InputResult::Handled)) => "handled".to_owned(),
        Ok(Some(InputResult::Continue) | None) => "continue".to_owned(),
        Err(message) => format!("error:{message}"),
    }
}

/// Drives the example through the controlled adapter.
struct Driver {
    host: Rc<ControlledHost>,
    session: Rc<Session>,
    ctx: ExtensionContext,
}

impl Driver {
    /// Runs the factory against a fresh controlled host.
    async fn start() -> Result<Self, Failure> {
        let host = ControlledHost::new();
        extension(ExtensionAPI::new(host.clone())).await?;
        let session = Session::new("/work");
        let ctx = host.context(&session, None);
        Ok(Self { host, session, ctx })
    }

    /// Dispatches one input and records the outcome.
    async fn input(&self, text: &str) -> Result<(), Failure> {
        let handler = present(self.host.input_handler(), "input handler")?;
        let event = InputEvent {
            text: text.to_owned(),
            images: None,
            source: InputSource::Interactive,
        };
        let outcome = run_input(&handler, event, self.ctx.clone()).await;
        self.host.note(format!(
            "input-outcome text={} decision={}",
            outcome.event.text,
            describe(&outcome.decision)
        ));
        Ok(())
    }

    /// Prepares tool arguments.
    fn prepare(&self, raw: &str) -> Result<String, Failure> {
        let tool = present(self.host.tool("echo"), "echo tool")?;
        let prepare = present(tool.prepare_arguments, "argument preparation")?;
        let prepared = prepare_arguments(&prepare, raw)?;
        self.host.note(format!("prepared {prepared}"));
        Ok(prepared)
    }

    /// Runs one tool call with a fresh signal and returns it.
    async fn tool(&self, call_id: &str, params: &str) -> Result<Rc<Flag>, Failure> {
        let tool = present(self.host.tool("echo"), "echo tool")?;
        let flag = Rc::new(Flag(Cell::new(false)));
        let call = ToolInvocation {
            call_id: call_id.to_owned(),
            params: params.to_owned(),
            signal: Some(AbortSignal::new(flag.clone())),
            update: Some(self.host.update_callback(call_id)),
        };
        let result = run_tool(&tool.execute, call, self.ctx.clone()).await?;
        let details = result.details.unwrap_or_default();
        self.host.note(format!(
            "tool-result {call_id} content={} details={details}",
            result.content.join(",")
        ));
        Ok(flag)
    }

    /// Runs the replace command, holding its new session open until the host observed it pending.
    async fn command(&self) -> Result<(), Failure> {
        let command = present(self.host.command("replace"), "replace command")?;
        let (started_tx, started) = oneshot::channel();
        let (open, opened) = oneshot::channel::<()>();
        self.host.hold_next_new_session(Box::new(move || {
            Box::pin(async move {
                let _ = started_tx.send(());
                let _ = opened.await;
            })
        }));
        let driver = async {
            started.await?;
            let last = self.host.transcript().last().cloned();
            assert_eq!(last.as_deref(), Some("new-session start parent=parent"));
            open.send(())
                .map_err(|()| Failure::from("the host stopped waiting"))
        };
        let call = command("go".to_owned(), self.host.command_context(&self.session));
        let (finished, gate) = tokio::join!(call, driver);
        gate?;
        Ok(finished?)
    }

    /// Renders the note message, invalidates it and drops the component.
    fn render(&self) -> Result<(), Failure> {
        let renderer = present(self.host.renderer("note"), "note renderer")?;
        let message = CustomMessage {
            custom_type: "note".to_owned(),
            content: "body".to_owned(),
            details: None,
        };
        let component = present(renderer(message, true)?, "component")?;
        self.host
            .note(format!("rendered {}", component.render(20).join(",")));
        component.invalidate();
        Ok(())
    }
}

/// The scenario both adapters run.
async fn scenario() -> Result<(), Failure> {
    let driver = Driver::start().await?;
    for text in ["hello", "reject"] {
        driver.input(text).await?;
    }
    let prepared = driver.prepare(r#"{"text":"x"}"#)?;
    let first = driver.tool("c1", &prepared).await?;
    first.0.set(true);
    driver.tool("c2", &prepared).await?;
    driver.command().await?;
    driver.render()?;
    let Driver { host, .. } = driver;
    host.release_all();
    assert_eq!(host.transcript(), scenario::EXPECTED);
    Ok(())
}

/// The controlled adapter produces the transcript the component produces.
#[test]
fn controlled_adapter_matches_the_component_transcript() -> Result<(), Failure> {
    tokio::runtime::Builder::new_current_thread()
        .build()?
        .block_on(scenario())
}
