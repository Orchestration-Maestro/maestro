//! Controlled command effects on native files and output sinks.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use maestro_models::catalog_generation::generate_models::generate_models;
use maestro_models::{
    Fetch, HttpResponse, Model, ModelCompat, ModelCost, ModelInput, ModelThinkingLevel,
};
use std::{
    fs,
    future::Future,
    io,
    path::PathBuf,
    sync::{Arc, Mutex},
};
/// Own native command output files.
struct Scratch(PathBuf);
impl Scratch {
    /// Allocate a unique command directory.
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "maestro-command-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
/// Build a controlled fetch whose request occurs after the previous body closes.
fn controlled(fail: Option<usize>) -> (Fetch, Arc<Mutex<Vec<String>>>) {
    controlled_with_bodies(fail, Vec::new())
}
/// Hold body completion until each caller-controlled release.
fn controlled_with_bodies(
    fail: Option<usize>,
    gates: Vec<tokio::sync::oneshot::Receiver<()>>,
) -> (Fetch, Arc<Mutex<Vec<String>>>) {
    let gates = Mutex::new(std::collections::VecDeque::from(gates));
    let events = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&events);
    let fetch: Fetch = Arc::new(move |request| {
        let position = {
            let mut events = recorded.lock().unwrap();
            assert_eq!(events.len() % 2, 0);
            events.push(request.url);
            events.len() / 2
        };
        let recorded = Arc::clone(&recorded);
        let gate = gates.lock().unwrap().pop_front();
        Box::pin(async move {
            if fail == Some(position) {
                recorded.lock().unwrap().push("failed".into());
                return Err(maestro_models::FetchError::Connection(
                    maestro_models::DiagnosticErrorInfo {
                        name: None,
                        message: "controlled failure".into(),
                        stack: None,
                        code: None,
                    },
                ));
            }
            let bytes = if position == 0 {
                b"{}".to_vec()
            } else {
                br#"{"data":[]}"#.to_vec()
            };
            let mut body = Some(bytes);
            Ok(HttpResponse {
                status: 200,
                status_text: String::new(),
                headers: std::collections::BTreeMap::new(),
                body: completed_body(body.take().unwrap(), recorded, gate),
            })
        })
    });
    (fetch, events)
}
/// Drive a command without a background runtime.
fn run(
    fetch: &Fetch,
    destination: &std::path::Path,
    output: &mut dyn io::Write,
    errors: &mut dyn io::Write,
) -> io::Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(generate_models(fetch, destination, output, errors))
}
/// Inspect provider output before accepting the success report.
struct ObserveFiles<'a> {
    /// Output directory.
    destination: &'a std::path::Path,
    /// Captured bytes.
    bytes: Vec<u8>,
}
impl io::Write for ObserveFiles<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.starts_with(b"Generated ") {
            assert!(self.destination.join("mod.rs").is_file());
            assert!(self.destination.join("provider_openai.rs").is_file());
        }
        self.bytes.extend(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn catalog_command_fetches_sequentially_and_reports_after_writes() {
    let scratch = Scratch::new();
    let destination = scratch.0.join("generated");
    let (release_dev, dev) = tokio::sync::oneshot::channel();
    let (release_router, router) = tokio::sync::oneshot::channel();
    let (fetch, events) = controlled_with_bodies(None, vec![dev, router]);
    let mut output = ObserveFiles {
        destination: &destination,
        bytes: Vec::new(),
    };
    let mut errors = Vec::new();
    observe_pending_bodies(
        generate_models(&fetch, &destination, &mut output, &mut errors),
        &events,
        [release_dev, release_router],
    );
    assert!(errors.is_empty());
    assert_eq!(
        *events.lock().unwrap(),
        [
            "https://models.dev/api.json",
            "body-complete",
            "https://openrouter.ai/api/v1/models",
            "body-complete",
            "https://ai-gateway.vercel.sh/v1/models",
            "body-complete"
        ]
    );
    assert_eq!(
        String::from_utf8(output.bytes).unwrap(),
        "Fetching models from models.dev API...\nLoaded 0 tool-capable models from models.dev\nFetching models from OpenRouter API...\nFetched 0 tool-capable models from OpenRouter\nFetching models from Vercel AI Gateway API...\nFetched 0 tool-capable models from Vercel AI Gateway\nGenerated src/catalog/models_generated/\n\nModel Statistics:\n  Total tool-capable models: 43\n  Reasoning-capable models: 36\n  amazon-bedrock: 1 models\n  anthropic: 3 models\n  google: 1 models\n  openai: 5 models\n  deepseek: 2 models\n  openai-codex: 10 models\n  xai: 1 models\n  mistral: 1 models\n  openrouter: 1 models\n  google-vertex: 13 models\n  azure-openai-responses: 5 models\n"
    );
}

/// Poll the same command through two explicitly held body-completion boundaries.
fn observe_pending_bodies(
    command: impl Future<Output = io::Result<()>>,
    events: &Mutex<Vec<String>>,
    releases: [tokio::sync::oneshot::Sender<()>; 2],
) {
    let [release_dev, release_router] = releases;
    let mut command = std::pin::pin!(command);
    let waker = futures_util::task::noop_waker();
    let mut context = std::task::Context::from_waker(&waker);
    assert!(command.as_mut().poll(&mut context).is_pending());
    assert_eq!(*events.lock().unwrap(), ["https://models.dev/api.json"]);
    release_dev.send(()).unwrap();
    assert!(command.as_mut().poll(&mut context).is_pending());
    assert_eq!(
        *events.lock().unwrap(),
        [
            "https://models.dev/api.json",
            "body-complete",
            "https://openrouter.ai/api/v1/models"
        ]
    );
    release_router.send(()).unwrap();
    assert!(matches!(
        command.as_mut().poll(&mut context),
        std::task::Poll::Ready(Ok(()))
    ));
}

#[test]
fn catalog_command_preserves_feed_failure_fallbacks() {
    for source in 0..3 {
        let scratch = Scratch::new();
        let (fetch, events) = controlled(Some(source));
        let mut output = Vec::new();
        let mut errors = Vec::new();
        run(
            &fetch,
            &scratch.0.join("generated"),
            &mut output,
            &mut errors,
        )
        .unwrap();
        assert_eq!(events.lock().unwrap().len(), 6);
        assert!(
            String::from_utf8(errors)
                .unwrap()
                .contains("controlled failure")
        );
        assert!(
            String::from_utf8(output)
                .unwrap()
                .contains("Total tool-capable models: 43\n")
        );
    }
}

/// Fail a writer at an exact byte boundary with an identifiable native cause.
struct FailingWriter {
    /// Accepted prefix.
    bytes: Vec<u8>,
    /// Remaining capacity.
    remaining: usize,
}
impl io::Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::from_raw_os_error(28));
        }
        let size = bytes.len().min(self.remaining);
        self.bytes.extend_from_slice(&bytes[..size]);
        self.remaining -= size;
        Ok(size)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn catalog_command_returns_write_and_output_errors() {
    let scratch = Scratch::new();
    let destination = scratch.0.join("generated");
    let (fetch, _) = controlled(None);
    let mut complete = Vec::new();
    run(&fetch, &destination, &mut complete, &mut Vec::new()).unwrap();
    let text = String::from_utf8(complete.clone()).unwrap();
    for phase in [
        "Fetching models from models.dev",
        "Loaded 0",
        "Fetching models from OpenRouter",
        "Fetched 0 tool-capable models from OpenRouter",
        "Fetching models from Vercel",
        "Fetched 0 tool-capable models from Vercel",
        "Generated ",
        "Model Statistics:",
        "  Total ",
        "  Reasoning-",
        "  anthropic:",
    ] {
        let limit = text.find(phase).unwrap();
        let mut writer = FailingWriter {
            bytes: Vec::new(),
            remaining: limit,
        };
        let (fetch, events) = controlled(None);
        let error = run(&fetch, &destination, &mut writer, &mut Vec::new()).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(28), "{phase}");
        assert_eq!(writer.bytes, complete[..limit], "{phase}");
        if limit == 0 {
            assert!(events.lock().unwrap().is_empty());
        }
    }
    for source in 0..3 {
        let (fetch, events) = controlled(Some(source));
        let error = run(
            &fetch,
            &destination,
            &mut Vec::new(),
            &mut FailingWriter {
                bytes: Vec::new(),
                remaining: 0,
            },
        )
        .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(28));
        assert_eq!(events.lock().unwrap().len(), 2 * (source + 1));
    }
    assert_filesystem_errors(&fetch);
}

/// Produce one additional router descriptor with externally supplied identity text.
fn custom_fetch(id: &str) -> Fetch {
    let router = serde_json::to_vec(&serde_json::json!({"data": [{"id": id, "name": "Name \"\\\n\u{0}😀", "supported_parameters": ["tools"], "architecture": {"modality": "text+image->text"}, "pricing": {"prompt": "0.000001", "completion": "0.000002"}}]})).unwrap();
    let dev = br#"{"cerebras":{"models":{"removable":{"tool_call":true}}}}"#.to_vec();
    Arc::new(move |request| {
        let bytes = if request.url.contains("openrouter") {
            router.clone()
        } else if request.url.contains("models.dev") {
            dev.clone()
        } else {
            br#"{"data":[]}"#.to_vec()
        };
        Box::pin(async move {
            Ok(HttpResponse {
                status: 200,
                status_text: String::new(),
                headers: std::collections::BTreeMap::new(),
                body: Box::pin(futures_util::stream::iter([Ok(bytes)])),
            })
        })
    })
}
/// Read every file as a closed, deterministic byte snapshot.
fn snapshot(directory: &std::path::Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fs::read_dir(directory)
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| entry.file_type().unwrap().is_file())
        .map(|entry| {
            (
                entry.file_name().into_string().unwrap(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect()
}
#[test]
fn catalog_files_have_one_owner_and_repeat_deterministically() {
    let scratch = Scratch::new();
    let destination = scratch.0.join("generated");
    let fetch = custom_fetch("extra-identity");
    run(&fetch, &destination, &mut Vec::new(), &mut Vec::new()).unwrap();
    fs::write(
        destination.join("provider_unrelated.rs"),
        "unrelated sentinel",
    )
    .unwrap();
    let first = snapshot(&destination);
    assert_eq!(first.len(), 14);
    assert!(destination.join("provider_cerebras.rs").is_file());
    run(&fetch, &destination, &mut Vec::new(), &mut Vec::new()).unwrap();
    assert_eq!(snapshot(&destination), first);
    let (empty, _) = controlled(None);
    run(&empty, &destination, &mut Vec::new(), &mut Vec::new()).unwrap();
    assert_eq!(
        fs::read_to_string(destination.join("provider_unrelated.rs")).unwrap(),
        "unrelated sentinel"
    );
    assert!(
        !fs::read_to_string(destination.join("provider_openrouter.rs"))
            .unwrap()
            .contains("extra-identity")
    );
    assert!(!destination.join("provider_cerebras.rs").exists());
    assert_eq!(snapshot(&destination).len(), 13); // eleven providers, registry and sentinel
}

#[test]
fn generated_catalog_source_compiles_and_reconstructs_descriptors() {
    let scratch = Scratch::new();
    let destination = scratch.0.join("generated");
    let identity = "boundary/\"\\\n\u{0}😀";
    let fetch = custom_fetch(identity);
    run(&fetch, &destination, &mut Vec::new(), &mut Vec::new()).unwrap();
    let model = boundary::cases()
        .into_iter()
        .find(|(key, _)| *key == "command-boundary")
        .unwrap()
        .1;
    let fixture = include_str!("fixtures/catalog_boundary.rs");
    let body = fixture
        .split("fn model_22() -> crate::Model {\n")
        .nth(1)
        .unwrap()
        .split("\n}\n}")
        .next()
        .unwrap();
    assert!(
        fs::read_to_string(destination.join("provider_openrouter.rs"))
            .unwrap()
            .contains(&format!("{body}\n}}"))
    );
    let expected: Model = serde_json::from_value(serde_json::json!({"id": identity, "name": "Name \"\\\n\u{0}😀", "api": "openai-completions", "provider": "openrouter", "baseUrl": "https://openrouter.ai/api/v1", "reasoning": false, "input": ["text", "image"], "cost": {"input": 1, "output": 2, "cacheRead": 0, "cacheWrite": 0}, "contextWindow": 4096, "maxTokens": 4096})).unwrap();
    assert_eq!(model, expected);
}

/// Close the body before recording the completion used by subsequent requests.
fn completed_body(
    bytes: Vec<u8>,
    recorded: Arc<Mutex<Vec<String>>>,
    mut gate: Option<tokio::sync::oneshot::Receiver<()>>,
) -> maestro_models::HttpBody {
    let mut body = Some(bytes);
    Box::pin(futures_util::stream::poll_fn(move |context| {
        if let Some(bytes) = body.take() {
            return std::task::Poll::Ready(Some(Ok(bytes)));
        }
        if let Some(receiver) = &mut gate {
            match std::pin::Pin::new(receiver).poll(context) {
                std::task::Poll::Pending => return std::task::Poll::Pending,
                std::task::Poll::Ready(result) => result.unwrap(),
            }
            gate = None;
        }
        recorded.lock().unwrap().push("body-complete".into());
        std::task::Poll::Ready(None)
    }))
}

/// Exercise native write failures without emitting a generation report.
fn assert_filesystem_errors(fetch: &Fetch) {
    for boundary in ["parent", "directory", "provider", "registry"] {
        let scratch = Scratch::new();
        let destination = scratch.0.join("generated");
        let destination = match boundary {
            "parent" => scratch.0.join("missing/generated"),
            "directory" => {
                fs::write(&destination, "not a directory").unwrap();
                destination
            }
            "provider" => {
                fs::create_dir(&destination).unwrap();
                fs::create_dir(destination.join("provider_amazon-bedrock.rs")).unwrap();
                destination
            }
            _ => {
                fs::create_dir(&destination).unwrap();
                fs::create_dir(destination.join("mod.rs")).unwrap();
                destination
            }
        };
        let mut output = Vec::new();
        let error = run(fetch, &destination, &mut output, &mut Vec::new()).unwrap_err();
        assert!(error.raw_os_error().is_some());
        assert!(!String::from_utf8(output).unwrap().contains("Generated "));
    }
}

/// Native boundary constructors compiled by Cargo with the command tests.
mod boundary {
    include!("fixtures/catalog_boundary.rs");
}
