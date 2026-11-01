//! Literal child operations observed through the real component interface.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CommandOptions, ExtensionAPI, ExtensionCommandContext, ExtensionContext, ExtensionFuture,
    ExtensionResult, Subscription,
    child_process::{ChildProcess, ExitStatus, SpawnOptions, Stdio},
    io::NativeIo,
    streams::{Readable, Writable},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, task::Poll};

/// Native resources retained across independent command callbacks.
#[derive(Default)]
pub(super) struct Slot {
    /// An explicit caller-owned subscription, not automatic spawn cancellation.
    abort: RefCell<Option<Subscription>>,
    /// Literal launch selected for the next agent-start callback.
    armed: RefCell<Option<(String, Vec<String>)>>,
    /// Original child controls.
    child: RefCell<Option<Rc<dyn ChildProcess>>>,
    /// One original admitted completion future.
    waiting: RefCell<Option<ExtensionFuture<'static, ExitStatus>>>,
    /// Input retained independently of child controls.
    stdin: RefCell<Option<Rc<dyn Writable>>>,
    /// Output retained independently of child controls.
    stdout: RefCell<Option<Rc<dyn Readable>>>,
    /// Error output retained independently of child controls.
    stderr: RefCell<Option<Rc<dyn Readable>>>,
}
/// A descriptor spelling accepted by the example command.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Descriptor {
    /// Retained pipe.
    Pipe,
    /// Native null device.
    Ignore,
    /// Corresponding inherited descriptor.
    Inherit,
}
impl From<Descriptor> for Stdio {
    fn from(value: Descriptor) -> Self {
        match value {
            Descriptor::Pipe => Self::Pipe,
            Descriptor::Ignore => Self::Ignore,
            Descriptor::Inherit => Self::Inherit,
        }
    }
}
/// Literal launch data, with every optional selection kept distinct.
#[derive(Deserialize)]
struct Launch {
    /// Program name.
    command: String,
    /// Literal argv.
    args: Vec<String>,
    /// Native directory selection.
    cwd: Option<String>,
    /// Complete optional environment.
    env: Option<Vec<(String, String)>>,
    /// Native session selection.
    #[serde(default)]
    detached: bool,
    /// Independent descriptor selections.
    stdio: Option<[Descriptor; 3]>,
}
/// One typed operation received by the example command.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Action {
    /// Selects explicit signal binding at the next active run.
    ArmAbort {
        /// Program name.
        command: String,
        /// Literal arguments.
        args: Vec<String>,
    },
    /// Reports whether the caller still owns its remover.
    Observer,
    /// Launches supplied operands.
    Spawn {
        /// Launch data.
        #[serde(flatten)]
        launch: Launch,
    },
    /// Reads a selected descriptor.
    Read {
        /// Chooses stderr instead of stdout.
        stderr: bool,
    },
    /// Admits exact input bytes.
    Write {
        /// Unmodified input.
        bytes: Vec<u8>,
    },
    /// Closes child input.
    End,
    /// Closes a consuming output.
    Close {
        /// Chooses stderr instead of stdout.
        stderr: bool,
    },
    /// Reports actual descriptor presence.
    Descriptors,
    /// Checks independently retained aliases of the same native resource.
    Aliases,
    /// Observes completion after output close.
    Wait,
    /// Polls and retains the same completion future.
    WaitStart,
    /// Awaits the retained original future.
    WaitFinish,
    /// Signals the original child.
    Kill {
        /// Omitted uses native ordinary termination.
        signal: Option<String>,
    },
    /// Sends an explicit PID or group signal.
    SignalPid {
        /// Native PID, negative for a supported group.
        pid: i32,
        /// Native signal name.
        signal: Option<String>,
    },
    /// Retains pipes independently of child controls.
    SavePipes,
    /// Releases independently retained pipes.
    DropPipes,
    /// Releases child controls without signalling.
    Drop,
}
impl Slot {
    /// Acquires the original child only when the selected operation needs it.
    fn child(&self) -> ExtensionResult<Rc<dyn ChildProcess>> {
        self.child
            .borrow()
            .clone()
            .ok_or("no retained child".into())
    }
    /// Selects an independently retained output, or the child's original output.
    fn output(&self, stderr: bool) -> ExtensionResult<Rc<dyn Readable>> {
        let saved = if stderr {
            self.stderr.borrow().clone()
        } else {
            self.stdout.borrow().clone()
        };
        if let Some(saved) = saved {
            return Ok(saved);
        }
        let child = self.child()?;
        (if stderr {
            child.stderr()
        } else {
            child.stdout()
        })
        .ok_or("missing pipe".into())
    }
    /// Selects independently retained input without resetting its native state.
    fn input(&self) -> ExtensionResult<Rc<dyn Writable>> {
        if let Some(saved) = self.stdin.borrow().clone() {
            return Ok(saved);
        }
        self.child()?.stdin().ok_or("missing stdin".into())
    }
}
/// Encodes status for observation only; no identity or stream crosses JSON.
fn status(status: &ExitStatus) -> Value {
    json!({"code":status.code,"signal":status.signal})
}

/// Executes a public resource operation or a literal native launch.
pub(super) async fn execute(
    io: Rc<dyn NativeIo>,
    slot: &Slot,
    context: &ExtensionCommandContext,
    data: &str,
) -> ExtensionResult<Value> {
    let action: Action = serde_json::from_str(data).map_err(|e| e.to_string())?;
    match action {
        Action::ArmAbort { command, args } => {
            *slot.armed.borrow_mut() = Some((command, args));
            Ok(Value::Null)
        }
        Action::Observer => Ok(json!(slot.abort.borrow().is_some())),
        Action::Spawn { launch } => {
            let options = SpawnOptions {
                cwd: launch.cwd,
                env: launch.env,
                detached: launch.detached,
                stdio: launch
                    .stdio
                    .map_or([Stdio::Pipe; 3], |stdio| stdio.map(Into::into)),
            };
            let child = io.spawn(launch.command, launch.args, options).await?;
            let pid = child.pid();
            *slot.child.borrow_mut() = Some(child);
            Ok(json!(pid))
        }
        Action::SignalPid { pid, signal } => {
            io.kill(pid, signal)?;
            Ok(Value::Null)
        }
        Action::Drop => {
            drop(slot.child.borrow_mut().take());
            Ok(Value::Null)
        }
        Action::DropPipes => {
            drop(slot.stdin.borrow_mut().take());
            drop(slot.stdout.borrow_mut().take());
            drop(slot.stderr.borrow_mut().take());
            Ok(Value::Null)
        }
        Action::Read { stderr } => Ok(json!(slot.output(stderr)?.read().await?)),
        Action::Close { stderr } => {
            slot.output(stderr)?.close()?;
            Ok(Value::Null)
        }
        Action::Write { bytes } => {
            slot.input()?.write(&bytes).await?;
            Ok(Value::Null)
        }
        Action::End => {
            slot.input()?.end().await?;
            Ok(Value::Null)
        }
        action => controls(slot, context, action).await,
    }
}
/// Observes or retains the original child without reconstructing resource state.
async fn controls(
    slot: &Slot,
    context: &ExtensionCommandContext,
    action: Action,
) -> ExtensionResult<Value> {
    if matches!(action, Action::WaitFinish) {
        let wait = slot.waiting.borrow_mut().take().ok_or("no retained wait")?;
        return settled(slot, wait.await);
    }
    let child = slot.child()?;
    match action {
        Action::Aliases => Ok(json!(
            Rc::ptr_eq(
                &child.stdin().ok_or("missing stdin")?,
                &child.stdin().ok_or("missing stdin")?
            ) && Rc::ptr_eq(
                &child.stdout().ok_or("missing stdout")?,
                &child.stdout().ok_or("missing stdout")?
            ) && Rc::ptr_eq(
                &child.stderr().ok_or("missing stderr")?,
                &child.stderr().ok_or("missing stderr")?
            )
        )),
        Action::Descriptors => Ok(json!([
            child.stdin().is_some(),
            child.stdout().is_some(),
            child.stderr().is_some()
        ])),
        Action::SavePipes => {
            *slot.stdin.borrow_mut() = child.stdin();
            *slot.stdout.borrow_mut() = child.stdout();
            *slot.stderr.borrow_mut() = child.stderr();
            Ok(Value::Null)
        }
        Action::Kill { signal } => Ok(json!(child.kill(signal)?)),
        Action::Wait => settled(slot, child.wait().await),
        Action::WaitStart => {
            let mut wait: ExtensionFuture<'static, ExitStatus> =
                Box::pin(async move { child.wait().await });
            let polled = std::future::poll_fn(|cx| Poll::Ready(wait.as_mut().poll(cx))).await;
            let pending = polled.is_pending();
            if pending {
                *slot.waiting.borrow_mut() = Some(wait);
            }
            context.get_system_prompt()?;
            Ok(json!(pending))
        }
        _ => Err("operation already handled".into()),
    }
}

/// Removes the original caller's observer even when native completion fails.
fn settled(slot: &Slot, result: ExtensionResult<ExitStatus>) -> ExtensionResult<Value> {
    let removed = slot
        .abort
        .borrow_mut()
        .take()
        .map_or(Ok(()), |subscription| subscription.unsubscribe());
    result.and_then(|status| removed.map(|()| self::status(&status)))
}

/// Registers commands and an explicit caller policy on the ordinary event owner.
pub(super) fn register(api: &ExtensionAPI, io: Rc<dyn NativeIo>) -> ExtensionResult<()> {
    let slot = Rc::new(Slot::default());
    let (commands, child) = (Rc::clone(&io), Rc::clone(&slot));
    let reporter = api.clone();
    api.register_command(
        "native-child",
        CommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(move |args, context| {
                let (io, slot) = (Rc::clone(&commands), Rc::clone(&child));
                let reporter = reporter.clone();
                Box::pin(async move {
                    let result = execute(io, &slot, &context, &args).await;
                    let value = match result {
                        Ok(value) => json!({"value":value}),
                        Err(error) => json!({"error":error}),
                    };
                    reporter.append_entry("native-io-result", Some(value.clone()))?;
                    context.ui()?.set_title(&format!("NATIVE-IO:{value}"))
                })
            }),
        },
    )?;
    api.on(
        "agent_start",
        Rc::new(move |_, context| {
            let (io, slot) = (Rc::clone(&io), Rc::clone(&slot));
            Box::pin(async move {
                observe_abort(io, &slot, &context).await?;
                Ok(None)
            })
        }),
    )
}

/// A user-installed callback signals its own pending child after this handler returns.
async fn observe_abort(
    io: Rc<dyn NativeIo>,
    slot: &Slot,
    context: &ExtensionContext,
) -> ExtensionResult<()> {
    let armed = slot.armed.borrow_mut().take();
    let Some((command, args)) = armed else {
        return Ok(());
    };
    let child = io.spawn(command, args, SpawnOptions::default()).await?;
    let signal = context.signal()?.ok_or("missing active-run signal")?;
    let observed = Rc::clone(&child);
    let subscription = signal.subscribe(Rc::new(move |_| observed.kill(None).map(|_| ())))?;
    *slot.child.borrow_mut() = Some(child);
    *slot.abort.borrow_mut() = Some(subscription);
    Ok(())
}
