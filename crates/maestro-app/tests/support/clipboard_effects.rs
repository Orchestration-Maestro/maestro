//! Strict corpus decoding and controlled clipboard effects.

use futures_util::{FutureExt, future::LocalBoxFuture};
use maestro_app::utils::{
    clipboard::{ClipboardOperations, ClipboardPlatform},
    clipboard_native::ClipboardModule,
};
use maestro_app::{OutputMode, SpawnSyncError, SpawnSyncOptions, copy_to_clipboard_with};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    future::Future,
    io,
    process::{ExitStatus, Output},
    task::{Context, Poll, Waker},
};
use tokio::{process::Command, sync::oneshot};

/// Complete consumed corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    pub copy: Vec<Case>,
    pub wayland: Vec<Wayland>,
    pub availability: Vec<Admission>,
}
/// Copy operation and observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub test: String,
    pub input: Input,
    pub expected: Expected,
}
/// Controlled inputs; absent switches retain their default.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Input {
    pub platform: String,
    pub env: BTreeMap<String, String>,
    pub native: String,
    pub text: String,
    #[serde(default)]
    pub fail: Vec<String>,
    #[serde(default)]
    pub spawn_fail: bool,
    #[serde(default)]
    pub stdout_error: bool,
    #[serde(default)]
    pub backpressure: bool,
    pub env_after_native: Option<BTreeMap<String, String>>,
}
/// Ordered trace, exact frame bytes and final outcome.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    pub trace: Vec<Value>,
    pub stdout: Vec<String>,
    pub result: Value,
}
/// Shared display predicate input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wayland {
    pub env: BTreeMap<String, String>,
    pub result: bool,
}
/// Native loader admission input and observed invocation count.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Admission {
    pub platform: String,
    pub env: BTreeMap<String, String>,
    pub load_fails: bool,
    pub loads: usize,
    pub present: bool,
}
/// Reads every row through strict typed records.
pub fn corpus() -> Corpus {
    serde_json::from_str(include_str!("../fixtures/clipboard_copy.json")).unwrap()
}
/// Platform conversion for the recorded host observations.
pub fn platform(name: &str) -> ClipboardPlatform {
    match name {
        "linux" => ClipboardPlatform::Linux,
        "darwin" => ClipboardPlatform::Darwin,
        "win32" => ClipboardPlatform::Win32,
        _ => ClipboardPlatform::Other,
    }
}
/// A single operation's effects; no process-global state is mutated.
pub struct Effects<'a> {
    pub case: &'a Case,
    pub env: RefCell<BTreeMap<String, String>>,
    pub trace: RefCell<Vec<Value>>,
    pub stdout: RefCell<Vec<String>>,
    pub release: RefCell<Option<oneshot::Receiver<()>>>,
    pub process_error: Cell<bool>,
}
impl ClipboardModule for Effects<'_> {
    fn set_text<'a>(&'a self, text: &'a str) -> LocalBoxFuture<'a, io::Result<()>> {
        async move {
            self.trace.borrow_mut().push(json!(["native", text]));
            if self.case.input.native == "fail" {
                return Err(io::Error::other("native unavailable"));
            }
            let receiver = self.release.borrow_mut().take();
            if let Some(receiver) = receiver {
                receiver.await.unwrap();
            }
            self.trace.borrow_mut().push(json!(["native-done"]));
            Ok(())
        }
        .boxed_local()
    }
}
impl ClipboardOperations for Effects<'_> {
    fn platform(&self) -> ClipboardPlatform {
        platform(&self.case.input.platform)
    }
    fn env(&self, name: &str) -> Option<String> {
        self.env.borrow().get(name).cloned()
    }
    fn clipboard(&self) -> Option<&dyn ClipboardModule> {
        (self.case.input.native != "absent").then_some(self)
    }
    fn spawn_sync(
        &self,
        command: &mut Command,
        options: SpawnSyncOptions<'_>,
    ) -> Result<Output, SpawnSyncError> {
        let command = command.as_std();
        let name = std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(|s| s.to_str().unwrap())
            .collect::<Vec<_>>()
            .join(" ");
        let expected_args: &[&str] = match command.get_program().to_str().unwrap() {
            "xclip" => &["-selection", "clipboard"],
            "xsel" => &["--clipboard", "--input"],
            "which" => &["wl-copy"],
            "pbcopy" | "clip" | "termux-clipboard-set" => &[],
            _ => panic!("unexpected helper"),
        };
        assert_eq!(command.get_args().collect::<Vec<_>>(), expected_args);
        let SpawnSyncOptions::Capture {
            stdout,
            input,
            timeout,
            stderr,
        } = options
        else {
            panic!("capture required")
        };
        assert_eq!(stdout, OutputMode::Discard);
        assert_eq!(stderr, OutputMode::Discard);
        let options = if name == "which wl-copy" {
            assert!(input.is_none());
            assert!(timeout.is_none());
            json!({"stdio":"ignore"})
        } else {
            assert_eq!(timeout, Some(std::time::Duration::from_millis(5000)));
            json!({"input":std::str::from_utf8(input.unwrap()).unwrap(),"timeout":5000,"stdio":["pipe","ignore","ignore"]})
        };
        self.trace.borrow_mut().push(json!(["exec", name, options]));
        if self.case.input.fail.contains(&name) && self.process_error.get() {
            return Err(SpawnSyncError::Timeout);
        }
        Ok(Output {
            status: status(!self.case.input.fail.contains(&name)),
            stdout: vec![],
            stderr: vec![],
        })
    }
    fn spawn(&self, command: Command, input: Option<Vec<u8>>) -> io::Result<()> {
        assert_eq!(command.as_std().get_program(), "wl-copy");
        assert_eq!(command.as_std().get_args().count(), 0);
        self.trace.borrow_mut().push(json!([
            "spawn",
            "wl-copy",
            String::from_utf8(input.unwrap()).unwrap()
        ]));
        if self.case.input.spawn_fail {
            Err(io::Error::other("startup unavailable"))
        } else {
            Ok(())
        }
    }
    fn write_stdout(&self, bytes: &[u8]) -> io::Result<()> {
        self.trace.borrow_mut().push(json!(["stdout"]));
        self.stdout
            .borrow_mut()
            .push(std::str::from_utf8(bytes).unwrap().to_owned());
        if self.case.input.stdout_error {
            return Err(io::Error::from_raw_os_error(123));
        }
        // A buffered writer accepts the complete frame without refusing the copy.
        if self.case.input.backpressure {
            use std::io::Write as _;
            let mut writer = io::BufWriter::with_capacity(bytes.len() + 1, Vec::new());
            writer.write_all(bytes)?;
            assert_eq!(writer.buffer(), bytes);
        }
        Ok(())
    }
}
/// Portable synthetic completed process status.
#[cfg(unix)]
fn status(success: bool) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    ExitStatus::from_raw(if success { 0 } else { 256 })
}
/// Portable synthetic completed process status.
#[cfg(windows)]
fn status(success: bool) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    ExitStatus::from_raw(u32::from(!success))
}
/// Replays the exact rows assigned to one behavior through the root export.
pub async fn replay(name: &str) {
    let corpus = corpus();
    let cases: Vec<_> = corpus
        .copy
        .iter()
        .filter(|case| case.test == name)
        .collect();
    assert!(!cases.is_empty());
    for case in cases {
        // Nonzero completion and process failure must enter the same fallback.
        for process_error in [false, true]
            .into_iter()
            .take(if case.input.fail.is_empty() { 1 } else { 2 })
        {
            replay_case(case, process_error).await;
        }
    }
}

/// Drives one controlled behavior without runtime attribute expansion.
pub fn run(name: &str) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(replay(name));
}

/// Drives one invocation and preserves its ordered observable transcript.
async fn replay_case(case: &Case, process_error: bool) {
    let (sender, receiver) = oneshot::channel();
    let pending = case.input.native == "pending";
    let effects = Effects {
        case,
        env: RefCell::new(case.input.env.clone()),
        trace: RefCell::default(),
        stdout: RefCell::default(),
        release: RefCell::new(pending.then_some(receiver)),
        process_error: Cell::new(process_error),
    };
    let mut copy = Box::pin(copy_to_clipboard_with(&case.input.text, &effects));
    if pending {
        let mut context = Context::from_waker(Waker::noop());
        assert!(matches!(copy.as_mut().poll(&mut context), Poll::Pending));
        assert_eq!(
            *effects.trace.borrow(),
            vec![json!(["native", case.input.text])]
        );
        assert!(effects.stdout.borrow().is_empty());
        if let Some(env) = &case.input.env_after_native {
            *effects.env.borrow_mut() = env.clone();
        }
        sender.send(()).unwrap();
    }
    let result = copy.await;
    let observed = match result {
        Ok(()) => json!("ok"),
        Err(error) if case.input.stdout_error => {
            assert_eq!(error.raw_os_error(), Some(123));
            json!({"error":"output unavailable"})
        }
        Err(error) => json!({"error":error.to_string()}),
    };
    assert_eq!(observed, case.expected.result, "{} outcome", case.id);
    assert_eq!(
        *effects.trace.borrow(),
        case.expected.trace,
        "{} trace",
        case.id
    );
    assert_eq!(
        *effects.stdout.borrow(),
        case.expected.stdout,
        "{} stdout",
        case.id
    );
}
