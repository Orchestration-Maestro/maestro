//! Strict image corpus and per-operation effect transcripts.
#![cfg(test)]
use crate::clipboard_status as completed;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::future::LocalBoxFuture;
use maestro_app::utils::{
    clipboard::{ClipboardOperations, ClipboardPlatform},
    clipboard_image::{
        ClipboardImageOperations, ReadClipboardImageOptions, read_clipboard_image_with,
    },
    clipboard_native::ClipboardModule,
};
use maestro_app::{OutputMode, SpawnSyncError, SpawnSyncOptions};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    future::Future,
    io,
    path::{Path, PathBuf},
    process::Output,
    task::{Context, Poll, Waker},
};
use tokio::{process::Command, sync::oneshot};

/// Every consumed fixture section.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    pub extensions: Vec<Extension>,
    pub cases: Vec<Case>,
}
/// MIME input and its suffix.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extension {
    pub input: String,
    pub expected: Option<String>,
}
/// One controlled public invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub test: String,
    pub input: Input,
    pub expected: Expected,
}
/// Host observations and effect responses.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Input {
    pub platform: String,
    pub env: BTreeMap<String, String>,
    pub kernel: Option<String>,
    pub native: String,
    pub commands: Vec<Invocation>,
    pub native_data: Option<Vec<u8>>,
    pub native_count: Option<usize>,
    pub temp_data: Option<Vec<u8>>,
    pub temp_count: Option<usize>,
    #[serde(default)]
    pub cleanup_error: bool,
    pub options: Option<String>,
    pub ambient_platform: Option<String>,
    pub ambient_env: Option<BTreeMap<String, String>>,
}
/// One native helper response or real bounded child.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Invocation {
    pub program: String,
    pub args: Option<Vec<String>>,
    pub response: Option<Response>,
    pub real_count: Option<usize>,
}
/// Raw helper stdout and completion.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub data: String,
    pub encoding: String,
    pub status: i32,
    pub error: bool,
}
/// Exact ordered trace and either complete bytes or a uniform large result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    pub result: Option<Value>,
    pub trace: Vec<Value>,
    pub mime: Option<String>,
    pub length: Option<usize>,
    pub fill: Option<u8>,
}
/// Strict parsing rejects unknown typed-envelope fields.
pub fn corpus() -> Corpus {
    serde_json::from_str(include_str!("../fixtures/clipboard_images.json")).unwrap()
}
use crate::clipboard_platform as recorded;
use recorded::platform;
/// A single transaction, with no shared mutable process state.
struct Effects<'a> {
    case: &'a Case,
    trace: RefCell<Vec<Value>>,
    next: RefCell<usize>,
    release: RefCell<Option<oneshot::Receiver<()>>>,
}
impl ClipboardModule for Effects<'_> {
    fn set_text<'a>(&'a self, _: &'a str) -> LocalBoxFuture<'a, io::Result<()>> {
        unreachable!("image reader must not write")
    }
    fn has_image(&self) -> io::Result<bool> {
        assert_ne!(self.case.input.native, "forbidden");
        self.trace.borrow_mut().push(json!(["hasImage"]));
        if self.case.input.native == "has-error" {
            return Err(io::Error::from_raw_os_error(123));
        }
        Ok(self.case.input.native != "no-image")
    }
    fn get_image_binary(&self) -> LocalBoxFuture<'_, io::Result<Option<Vec<u8>>>> {
        Box::pin(async {
            self.trace.borrow_mut().push(json!(["getImageBinary"]));
            if self.case.input.native == "get-error" {
                return Err(io::Error::from_raw_os_error(123));
            }
            let receiver = self.release.borrow_mut().take();
            if let Some(receiver) = receiver {
                receiver.await.unwrap();
            }
            Ok(self
                .case
                .input
                .native_count
                .map(|count| vec![7; count])
                .or_else(|| self.case.input.native_data.clone()))
        })
    }
}
impl ClipboardOperations for Effects<'_> {
    fn platform(&self) -> ClipboardPlatform {
        platform(
            self.case
                .input
                .ambient_platform
                .as_ref()
                .unwrap_or(&self.case.input.platform),
        )
    }
    fn clipboard(&self) -> Option<&dyn ClipboardModule> {
        (self.case.input.native != "absent").then_some(self)
    }
    fn spawn(&self, _: Command, _: Option<Vec<u8>>) -> io::Result<()> {
        unreachable!("image commands must complete")
    }
    fn write_stdout(&self, _: &[u8]) -> io::Result<()> {
        unreachable!("images emit no terminal output")
    }
    fn spawn_sync(
        &self,
        command: &mut Command,
        options: SpawnSyncOptions<'_>,
    ) -> Result<Output, SpawnSyncError> {
        let native = command.as_std();
        let program = native.get_program().to_str().unwrap();
        let args: Vec<_> = native
            .get_args()
            .map(|arg| arg.to_str().unwrap().to_owned())
            .collect();
        assert_eq!(native.get_envs().count(), 0);
        let SpawnSyncOptions::Capture {
            stdout,
            stderr,
            input,
            timeout,
            max_buffer_bytes,
        } = options
        else {
            panic!("capture required")
        };
        assert_eq!(stdout, OutputMode::Capture);
        assert_eq!(stderr, OutputMode::Capture);
        assert_eq!(input, Some(&[][..]));
        assert_eq!(max_buffer_bytes, Some(50 * 1024 * 1024));
        let millis = timeout.unwrap().as_millis();
        self.trace.borrow_mut().push(json!(["command",program,args,{"timeout":millis,"maxBuffer":max_buffer_bytes,"env":null}]));
        let index = *self.next.borrow();
        *self.next.borrow_mut() += 1;
        let expected = &self.case.input.commands[index];
        assert_eq!(program, expected.program);
        if let Some(expected_args) = &expected.args {
            assert_eq!(&args, expected_args);
        } else {
            assert_eq!(program, "powershell.exe");
        }
        if let Some(count) = expected.real_count {
            return real_capture(count, options);
        }
        let response = expected.response.as_ref().unwrap();
        if response.error {
            return Err(SpawnSyncError::Timeout);
        }
        assert!(matches!(response.status, 0 | 1));
        let bytes = match response.encoding.as_str() {
            "utf8" => response.data.as_bytes().to_vec(),
            "base64" => STANDARD.decode(&response.data).unwrap(),
            _ => panic!("unknown encoding"),
        };
        Ok(Output {
            status: completed::status(response.status == 0),
            stdout: bytes,
            stderr: vec![],
        })
    }
}
/// Real byte collection proves the controlled command's buffer boundary.
#[cfg(unix)]
fn real_capture(count: usize, mut options: SpawnSyncOptions<'_>) -> Result<Output, SpawnSyncError> {
    // Exercise the byte boundary without racing runner speed against the deadline.
    if let SpawnSyncOptions::Capture { timeout, .. } = &mut options {
        *timeout = None;
    }
    let mut command = Command::new("python3");
    command.args([
        "-c",
        "import os,sys; os.write(1,b'\x07'*int(sys.argv[1]))",
        &count.to_string(),
    ]);
    maestro_process::spawn_sync(&mut command, options)
}
/// Windows compilation retains policy cases without a Python dependency.
#[cfg(windows)]
fn real_capture(count: usize, options: SpawnSyncOptions<'_>) -> Result<Output, SpawnSyncError> {
    let SpawnSyncOptions::Capture {
        max_buffer_bytes, ..
    } = options
    else {
        unreachable!()
    };
    if count > max_buffer_bytes.unwrap() {
        return Err(SpawnSyncError::Wait(io::Error::other("output limit")));
    }
    Ok(Output {
        status: completed::status(true),
        stdout: vec![7; count],
        stderr: vec![],
    })
}
/// Deterministic path used only in the controlled adapter.
const TEMP: &str = "/oracle/tmp/maestro-wsl-clip-00000000-0000-4000-8000-000000000001.png";
impl ClipboardImageOperations for Effects<'_> {
    fn read_file(&self, path: &Path) -> io::Result<Vec<u8>> {
        if path == Path::new("/proc/version") {
            self.trace
                .borrow_mut()
                .push(json!(["read", "/proc/version", "utf-8"]));
            return self
                .case
                .input
                .kernel
                .as_ref()
                .map(|text| text.as_bytes().to_vec())
                .ok_or_else(|| io::Error::other("kernel unavailable"));
        }
        assert_eq!(path, Path::new(TEMP));
        self.trace.borrow_mut().push(json!(["read", TEMP, null]));
        self.case
            .input
            .temp_count
            .map(|count| vec![7; count])
            .or_else(|| self.case.input.temp_data.clone())
            .ok_or_else(|| io::Error::other("temporary image unavailable"))
    }
    fn remove_file(&self, path: &Path) -> io::Result<()> {
        assert_eq!(path, Path::new(TEMP));
        self.trace.borrow_mut().push(json!(["remove", TEMP]));
        if self.case.input.cleanup_error {
            Err(io::Error::other("cleanup unavailable"))
        } else {
            Ok(())
        }
    }
    fn temporary_png_path(&self) -> PathBuf {
        self.trace
            .borrow_mut()
            .push(json!(["temporary-png-path", TEMP]));
        PathBuf::from(TEMP)
    }
}
/// Executes one named behavior's rows; a row that declares an ambient environment runs
/// in a cleared child holding it.
pub fn run(name: &str) {
    let cases: Vec<Case> = corpus()
        .cases
        .into_iter()
        .filter(|case| case.test == name)
        .collect();
    assert!(!cases.is_empty());
    let (here, ambient): (Vec<_>, Vec<_>) = cases
        .into_iter()
        .partition(|case| case.input.ambient_env.is_none());
    let selected = maestro_test_process::run_per_environment(name, ambient, |case| {
        case.input.ambient_env.clone().unwrap_or_default()
    })
    .unwrap();
    for case in selected.unwrap_or(here) {
        replay(&case);
    }
}
/// Drives a pending native read before releasing it and checking its complete result.
fn replay(case: &Case) {
    let (sender, receiver) = oneshot::channel();
    let pending = case.input.native == "pending";
    let effects = Effects {
        case,
        trace: RefCell::default(),
        next: RefCell::new(0),
        release: RefCell::new(pending.then_some(receiver)),
    };
    let mode = case.input.options.as_deref();
    let options = ReadClipboardImageOptions {
        env: (!matches!(mode, Some("default" | "empty" | "platform-only")))
            .then(|| case.input.env.clone()),
        platform: (!matches!(mode, Some("default" | "empty" | "env-only")))
            .then(|| platform(&case.input.platform)),
    };
    let mut read = Box::pin(read_clipboard_image_with(
        (mode != Some("default")).then_some(&options),
        &effects,
    ));
    if pending {
        assert!(matches!(
            read.as_mut().poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        assert_eq!(
            *effects.trace.borrow(),
            vec![json!(["hasImage"]), json!(["getImageBinary"])]
        );
        sender.send(()).unwrap();
    }
    let result = crate::support::run_io(read);
    check_result(case, result);
    assert_eq!(
        *effects.trace.borrow(),
        case.expected.trace,
        "{} trace",
        case.id
    );
    assert_eq!(
        *effects.next.borrow(),
        case.input.commands.len(),
        "{} unused command",
        case.id
    );
}
/// Retains bytes, ordering and native error identity when comparing the oracle.
fn check_result(
    case: &Case,
    result: io::Result<Option<maestro_app::utils::clipboard_image::ClipboardImage>>,
) {
    if let Some(length) = case.expected.length {
        let image = result.unwrap().unwrap();
        assert_eq!(image.mime_type, *case.expected.mime.as_ref().unwrap());
        assert_eq!(image.bytes.len(), length);
        assert!(
            image
                .bytes
                .iter()
                .all(|byte| Some(*byte) == case.expected.fill)
        );
        return;
    }
    let observed = match result {
        Err(error) => {
            assert_eq!(error.raw_os_error(), Some(123));
            json!({"error":case.input.native})
        }
        Ok(None) => Value::Null,
        Ok(Some(image)) => {
            if case.id == "raw-orientation" {
                let expected_bytes: Vec<u8> =
                    serde_json::from_value(case.expected.result.as_ref().unwrap()["bytes"].clone())
                        .unwrap();
                compare_pixels(&image.bytes, &expected_bytes);
                assert_eq!(
                    image.mime_type,
                    case.expected.result.as_ref().unwrap()["mime"]
                        .as_str()
                        .unwrap()
                );
                return;
            }
            json!({"mime":image.mime_type,"bytes":image.bytes})
        }
    };
    assert_eq!(
        observed,
        case.expected.result.clone().unwrap_or(Value::Null),
        "{} result",
        case.id
    );
}
/// Converted JPEG pixels use per-channel tolerance without rotating the image.
fn compare_pixels(actual: &[u8], expected: &[u8]) {
    assert_eq!(
        image::guess_format(actual).unwrap(),
        image::ImageFormat::Png
    );
    let actual = image::load_from_memory(actual).unwrap().to_rgba8();
    let expected = image::load_from_memory(expected).unwrap().to_rgba8();
    assert_eq!(actual.dimensions(), expected.dimensions());
    assert!(
        actual
            .as_raw()
            .iter()
            .zip(expected.as_raw())
            .all(|(a, b)| a.abs_diff(*b) <= 3)
    );
}

/// Checks that an expression-mode path stays quoted even when it is a safe argument.
pub fn safe_powershell_literal() {
    let mut case = corpus()
        .cases
        .into_iter()
        .find(|case| case.id == "powershell-image")
        .unwrap();
    let command = case
        .input
        .commands
        .iter_mut()
        .find(|command| command.program == "wslpath")
        .unwrap();
    command.response.as_mut().unwrap().data = "safe.png\n".into();
    let invocation = case
        .expected
        .trace
        .iter_mut()
        .find(|row| row[0] == "command" && row[1] == "powershell.exe")
        .unwrap();
    let script = invocation[2][2].as_str().unwrap();
    let original = r"$path = 'C:\Users\O''Hare\clip.png';";
    assert!(script.contains(original));
    invocation[2][2] = script.replace(original, "$path = 'safe.png';").into();
    replay(&case);
}
