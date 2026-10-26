//! Built-in native tools retained and invoked by a compiled extension author.
use serde_json::json;

#[test]
fn builtin_owned_edit_prepares_before_execution() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let runner = super::builtin_author::loaded().await;
        let command = runner
            .get_command("builtin-prepare-edit")
            .expect("compiled guest retains the native edit preparation")
            .command;
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("edit_preparation.json")).unwrap();
        assert_eq!(cases.len(), 22);
        for case in cases {
            let result =
                (command.handler)(case["input"].to_string(), runner.create_command_context()).await;
            let observed: serde_json::Value =
                serde_json::from_str(&result.unwrap_err().to_string()).unwrap();
            assert_eq!(observed, case["expected"], "{}", case["id"]);
        }
    });
}

#[test]
fn builtin_owned_tool_survives_factory_and_callback_return() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let path = scratch.path().join("read.txt");
        std::fs::write(&path, "native\0é😀\n").unwrap();
        let runner = super::builtin_author::loaded().await;
        let command = runner
            .get_command("builtin-read")
            .expect("compiled guest retains the native factory result")
            .command;
        for expected in ["native\0é😀\n", "changed after factory return\n"] {
            std::fs::write(&path, expected).unwrap();
            let result = (command.handler)(
                path.to_str().unwrap().into(),
                runner.create_command_context(),
            )
            .await;
            let text = result.unwrap_err().to_string();
            let observed: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(
                observed,
                json!({"content":[{"type":"text","text":expected}]})
            );
        }
    });
}

/// The author factory supplies every read option and byte-oriented reverse observation.
#[test]
fn builtin_read_options_preserve_reverse_operations() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let runner = super::builtin_author::loaded().await;
        let command = runner.get_command("bridge-observe").unwrap().command;
        for (detect, mime, stages) in [
            (false, serde_json::Value::Null, vec!["access", "read"]),
            (
                true,
                serde_json::Value::Null,
                vec!["access", "mime", "read"],
            ),
            (true, json!(""), vec!["access", "mime", "read"]),
            (true, json!("image/png"), vec!["access", "mime", "read"]),
        ] {
            let (expected, prepares) = native_read_result(mime.as_str().map(str::to_owned)).await;
            let request = json!({"detect":detect,"mime":mime,"bytes":[0,255,42],"resize":false});
            let result = (command.handler)(
                format!("builtin-custom-read {request}"),
                runner.create_command_context(),
            )
            .await
            .unwrap_err()
            .to_string();
            let observed: serde_json::Value = serde_json::from_str(&result).unwrap();
            let calls: Vec<_> = stages
                .into_iter()
                .map(|stage| json!([stage, "/virtual/雪.bin"]))
                .collect();
            assert_eq!(
                observed,
                json!({"result":expected,"calls":calls,"prepares":prepares}),
                "{request}"
            );
        }
    });
}

/// Ready and pending failures stop at their stage; cancellation wins over pending work.
#[test]
fn builtin_read_operation_failures_stop_at_the_reached_stage() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let runner = super::builtin_author::loaded().await;
        for stage in ["access", "mime", "read"] {
            for mode in ["ready", "pending", "preabort", "pending-abort"] {
                observe_read_failure(&runner, stage, mode).await;
            }
        }
    });
}

/// Observes cancellation through an arbitrary author port, not a sampled aborted Boolean.
async fn observe_read_failure(
    runner: &maestro_extensions::ExtensionRunner<maestro_extensions::Local>,
    stage: &str,
    mode: &str,
) {
    let signal = read_signal(runner);
    let command = runner.get_command("bridge-observe").unwrap().command;
    let request = json!({"detect":true,"mime":null,"bytes":[],"fail":stage,
        "gate":mode.starts_with("pending").then_some(stage),
        "signal":match mode { "preabort" => Some("preaborted"), "pending-abort" => Some("relay"), _ => None }});
    let result = if mode.starts_with("pending") {
        let (started, release) = read_gate(runner);
        let waiting = Box::pin((command.handler)(
            format!("builtin-custom-read {request}"),
            runner.create_command_context(),
        ));
        let waiting = match futures_util::future::select(waiting, started).await {
            futures_util::future::Either::Left(_) => {
                panic!("read completed before the {stage} operation gate")
            }
            futures_util::future::Either::Right((started, waiting)) => {
                started.unwrap();
                waiting
            }
        };
        if mode == "pending-abort" {
            signal.abort();
        }
        release.send(()).unwrap();
        waiting.await
    } else {
        (command.handler)(
            format!("builtin-custom-read {request}"),
            runner.create_command_context(),
        )
        .await
    };
    let observed: serde_json::Value =
        serde_json::from_str(&result.unwrap_err().to_string()).unwrap();
    assert_eq!(
        observed["error"],
        json!(native_read_failure(&request, signal, mode).await)
    );
    assert_eq!(
        observed,
        expected_read_failure(stage, mode),
        "{stage}/{mode}"
    );
}

/// Gives the author a native signal it must wrap through its own signal port.
fn read_signal(
    runner: &maestro_extensions::ExtensionRunner<maestro_extensions::Local>,
) -> maestro_extensions::Cancellation {
    let signal = maestro_extensions::Cancellation::new();
    let selected = signal.clone();
    runner
        .bind_core(
            <maestro_extensions::Local as maestro_extensions::ExtensionExecution>::unbound_actions(
            ),
            maestro_extensions::ExtensionContextActions {
                get_signal: std::rc::Rc::new(move || Ok(Some(selected.clone()))),
                ..Default::default()
            },
            maestro_extensions::types::ProviderActions::default(),
        )
        .unwrap();
    signal
}

/// Checks final read errors against the native wrapper; diagnostic codes remain native data.
async fn native_read_failure(
    request: &serde_json::Value,
    signal: maestro_extensions::Cancellation,
    mode: &str,
) -> String {
    if mode == "preabort" {
        signal.abort();
    }
    let error = native_read_outcome(request, None, Some(signal))
        .await
        .unwrap_err();
    if !mode.ends_with("abort") {
        assert_eq!(
            error.code,
            Some(maestro_extensions::DiagnosticCode::Number(17.0))
        );
    }
    error.to_string()
}

/// Literal reached stages and public read errors for each cancellation phase.
fn expected_read_failure(stage: &str, mode: &str) -> serde_json::Value {
    let mut stages = Vec::new();
    if mode != "preabort" {
        for reached in ["access", "mime", "read"] {
            stages.push(json!([reached, "/virtual/雪.bin"]));
            if reached == stage {
                break;
            }
        }
    }
    let error = if mode.ends_with("abort") {
        "Operation aborted".into()
    } else {
        format!("refused {stage} 雪")
    };
    let mut expected = json!({"error":error,"calls":stages,"prepares":false});
    if mode == "pending-abort" {
        expected["notified"] = json!(1);
    }
    if mode == "preabort" {
        expected["notified"] = json!(0);
    }
    expected
}

/// Supplies the same observations to the actual native factory used as the comparator.
struct ReadAnswer(Option<String>);
/// Native read callbacks retain their original thread-safe future contract.
type ReadFuture<T> = std::pin::Pin<
    Box<
        dyn std::future::Future<Output = Result<T, maestro_extensions::DiagnosticErrorInfo>> + Send,
    >,
>;
impl maestro_extensions::ReadOperations for ReadAnswer {
    fn access(&self, _: String) -> ReadFuture<()> {
        Box::pin(std::future::ready(Ok(())))
    }
    fn read_file(&self, _: String) -> ReadFuture<Vec<u8>> {
        Box::pin(std::future::ready(Ok(vec![0, 255, 42])))
    }
    fn detect_image_mime_type(&self, _: String) -> ReadFuture<Option<String>> {
        Box::pin(std::future::ready(Ok(self.0.clone())))
    }
}
/// Uses the existing native factory, never a duplicate expectation for its output messages.
async fn native_read_result(mime: Option<String>) -> (serde_json::Value, bool) {
    let native = maestro_extensions::local::create_read_tool_definition::<()>(
        "/virtual".into(),
        maestro_extensions::ReadToolOptions {
            auto_resize_images: Some(false),
            operations: Some(std::sync::Arc::new(ReadAnswer(mime))),
        },
    );
    let prepares = native.prepare_arguments.is_some();
    let native = maestro_extensions::local::wrap_tool_definition(native, None);
    let result = (native.execute)("native-read".into(), json!({"path":"雪.bin"}), None, None)
        .await
        .unwrap();
    (serde_json::to_value(result).unwrap(), prepares)
}

/// The executable carries native metadata and runs after factory return without a context.
#[test]
fn builtin_read_executable_keeps_native_metadata_without_context() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let path = scratch.path().join("executable.txt");
        std::fs::write(&path, "four arguments 雪\0\n").unwrap();
        let native = maestro_extensions::local::wrap_tool_definition(
            maestro_extensions::local::create_read_tool_definition::<()>("/".into(), maestro_extensions::ReadToolOptions::default()), None);
        let mut metadata = serde_json::to_value(&native.definition).unwrap();
        metadata["label"] = json!(native.label);
        if let Some(mode) = native.execution_mode {
            metadata["executionMode"] = serde_json::to_value(mode).unwrap();
        }
        let expected = (native.execute)("native".into(), json!({"path":path}), None, None).await.unwrap();
        let runner = super::builtin_author::loaded().await;
        let calls = std::rc::Rc::new(std::cell::Cell::new(0));
        let observed = calls.clone();
        runner.bind_core(
            <maestro_extensions::Local as maestro_extensions::ExtensionExecution>::unbound_actions(),
            maestro_extensions::ExtensionContextActions {
                get_model: std::rc::Rc::new(move || {
                    observed.set(observed.get() + 1);
                    Err("executable must not read context".into())
                }),
                ..Default::default()
            },
            maestro_extensions::types::ProviderActions::default(),
        ).unwrap();
        let command = runner.get_command("builtin-read-executable").unwrap().command;
        let result = (command.handler)(path.to_str().unwrap().into(), runner.create_command_context()).await;
        let observed: serde_json::Value = serde_json::from_str(&result.unwrap_err().to_string()).unwrap();
        assert_eq!(observed, json!({"metadata":metadata,"prepares":native.prepare_arguments.is_some(),"result":expected}));
        assert_eq!(calls.get(), 0);
    });
}

/// Raw bytes and MIME branches use only the supplied context, never the runner's replacement.
#[test]
fn builtin_read_operations_keep_bytes_mime_and_context() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let runner = super::builtin_author::loaded().await;
        for (detect, mime, bytes) in [
            (false, None, vec![0, 0xe9, 0x9b, 0xaa, 255, 42]),
            (true, None, vec![0, 255, 42]),
            (true, Some(""), "é😀\0\n".as_bytes().to_vec()),
            (true, Some("image/png"), vec![0, 255, 42]),
            (true, Some("image/jpeg"), vec![0, 255, 42]),
        ] {
            for (vision, executable) in [(None,false), (None,true), (Some(false),false), (Some(false),true), (Some(true),false), (Some(true),true)] {
                let request = json!({"detect":detect,"mime":mime,"bytes":bytes,"resize":false,"executable":executable});
                observe_read_case(&runner, request, vision, executable).await;
            }
        }
    });
}

/// Runs a single read with a supplied model reader and a distinct replacement binding.
async fn observe_read_case(
    runner: &maestro_extensions::ExtensionRunner<maestro_extensions::Local>,
    request: serde_json::Value,
    vision: Option<bool>,
    executable: bool,
) {
    let command = runner.get_command("bridge-observe").unwrap().command;
    let model = read_model(vision);
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let seen = calls.clone();
    let supplied = model.clone();
    bind_read_model(
        runner,
        std::rc::Rc::new(move || {
            seen.set(seen.get() + 1);
            if executable {
                return Err("executable must not ask for a model".into());
            }
            Ok(supplied.clone())
        }),
    );
    let context = runner.create_command_context();
    bind_read_model(
        runner,
        std::rc::Rc::new(|| Err("wrong replacement context".into())),
    );
    let expected = native_read_case(&request, if executable { None } else { model }).await;
    let result = (command.handler)(format!("builtin-custom-read {request}"), context).await;
    let observed: serde_json::Value =
        serde_json::from_str(&result.unwrap_err().to_string()).unwrap();
    let mut stages = vec!["access"];
    if request["detect"] == true {
        stages.push("mime");
    }
    stages.push("read");
    let trace: Vec<_> = stages
        .into_iter()
        .map(|stage| json!([stage, "/virtual/雪.bin"]))
        .collect();
    assert_eq!(
        observed,
        json!({"result":expected,"calls":trace,"prepares":false}),
        "{request}, vision={vision:?}"
    );
    assert_eq!(calls.get(), u32::from(!executable));
}

/// Installs a deliberately independent runner model reader for subsequent contexts only.
fn bind_read_model(
    runner: &maestro_extensions::ExtensionRunner<maestro_extensions::Local>,
    reader: std::rc::Rc<
        dyn Fn() -> Result<Option<maestro_extensions::Model>, maestro_extensions::EventError>,
    >,
) {
    runner
        .bind_core(
            <maestro_extensions::Local as maestro_extensions::ExtensionExecution>::unbound_actions(
            ),
            maestro_extensions::ExtensionContextActions {
                get_model: std::rc::Rc::new(move || {
                    reader().map(|model| {
                        model.map(|model| std::sync::Arc::new(std::sync::RwLock::new(model)))
                    })
                }),
                ..Default::default()
            },
            maestro_extensions::types::ProviderActions::default(),
        )
        .unwrap();
}

/// Distinguishes absence, a text-only model and an image-capable model.
fn read_model(vision: Option<bool>) -> Option<maestro_extensions::Model> {
    vision.map(|vision| {
        let handle = super::model_support::model("supplied-read-model");
        let mut model = handle.read().unwrap().clone();
        model.input = vec![maestro_models::ModelInput::Text];
        if vision {
            model.input.push(maestro_models::ModelInput::Image);
        }
        model
    })
}

/// Uses the public native factory and wrapper with the same authored observations.
async fn native_read_case(
    request: &serde_json::Value,
    model: Option<maestro_extensions::Model>,
) -> serde_json::Value {
    native_read_outcome(request, model, None).await.unwrap()
}

/// Invokes the native wrapper without flattening its structured error first.
async fn native_read_outcome(
    request: &serde_json::Value,
    model: Option<maestro_extensions::Model>,
    signal: Option<maestro_extensions::Cancellation>,
) -> Result<serde_json::Value, maestro_extensions::DiagnosticErrorInfo> {
    let options = maestro_extensions::ReadToolOptions {
        auto_resize_images: request["resize"].as_bool(),
        operations: Some(std::sync::Arc::new(ReadCase {
            bytes: serde_json::from_value(request["bytes"].clone()).unwrap(),
            mime: request["mime"].as_str().map(str::to_owned),
            fail: request["fail"].as_str().map(str::to_owned),
        })),
    };
    let definition = maestro_extensions::local::create_read_tool_definition::<ReadModel>(
        "/virtual".into(),
        options,
    );
    let factory = model.map(|model| {
        std::rc::Rc::new(move || Ok(ReadModel(model.clone())))
            as maestro_extensions::local::ToolContextFactory<ReadModel>
    });
    let tool = maestro_extensions::local::wrap_tool_definition(definition, factory);
    let result =
        (tool.execute)("comparator".into(), json!({"path":"雪.bin"}), signal, None).await?;
    Ok(serde_json::to_value(result).unwrap())
}

/// Native callback responses preserve the exact original bytes.
struct ReadCase {
    bytes: Vec<u8>,
    mime: Option<String>,
    fail: Option<String>,
}
impl ReadCase {
    /// Supplies the same authored diagnostic to the native read, without any guest transport.
    fn answer<T>(
        &self,
        stage: &str,
        value: T,
    ) -> Result<T, maestro_extensions::DiagnosticErrorInfo> {
        if self.fail.as_deref() == Some(stage) {
            return Err(maestro_extensions::DiagnosticErrorInfo {
                name: Some("ReadFailure".into()),
                message: format!("refused {stage} 雪"),
                stack: Some("authored stack".into()),
                code: Some(maestro_extensions::DiagnosticCode::Number(17.0)),
            });
        }
        Ok(value)
    }
}
impl maestro_extensions::ReadOperations for ReadCase {
    fn access(&self, _: String) -> ReadFuture<()> {
        Box::pin(std::future::ready(self.answer("access", ())))
    }
    fn read_file(&self, _: String) -> ReadFuture<Vec<u8>> {
        Box::pin(std::future::ready(self.answer("read", self.bytes.clone())))
    }
    fn detect_image_mime_type(&self, _: String) -> ReadFuture<Option<String>> {
        Box::pin(std::future::ready(self.answer("mime", self.mime.clone())))
    }
}
/// Context supplied to the native comparator, independent of the extension runner.
struct ReadModel(maestro_extensions::Model);
impl maestro_extensions::ReadContext for ReadModel {
    fn model(&self) -> Option<std::sync::Arc<maestro_extensions::Model>> {
        Some(std::sync::Arc::new(self.0.clone()))
    }
}

/// Image fitting and the call-time model are independent, including undecodable input.
#[test]
fn builtin_images_keep_resize_and_model_policy_independent() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let runner = super::builtin_author::loaded().await;
        for resize in [None, Some(true), Some(false)] {
            for vision in [None, Some(false), Some(true)] {
                observe_image_case(&runner, resize, vision).await;
            }
        }
    });
}

/// Changes the supplied model only after construction, before the definition is invoked.
async fn observe_image_case(
    runner: &maestro_extensions::ExtensionRunner<maestro_extensions::Local>,
    resize: Option<bool>,
    vision: Option<bool>,
) {
    let command = runner.get_command("bridge-observe").unwrap().command;
    for bytes in [include_bytes!("read_resize.png").as_slice(), &[0, 255, 42]] {
        let current = std::rc::Rc::new(std::cell::RefCell::new(None));
        let selected = current.clone();
        bind_read_model(
            runner,
            std::rc::Rc::new(move || Ok(selected.borrow().clone())),
        );
        let (started, release) = read_gate(runner);
        let request =
            json!({"detect":true,"mime":"image/png","bytes":bytes,"resize":resize,"before":true});
        let waiting = Box::pin((command.handler)(
            format!("builtin-custom-read {request}"),
            runner.create_command_context(),
        ));
        let waiting = match futures_util::future::select(waiting, started).await {
            futures_util::future::Either::Left(_) => {
                panic!("read completed before the post-construction gate")
            }
            futures_util::future::Either::Right((started, waiting)) => {
                started.unwrap();
                waiting
            }
        };
        let model = read_model(vision);
        *current.borrow_mut() = model.clone();
        release.send(()).unwrap();
        let observed: serde_json::Value =
            serde_json::from_str(&waiting.await.unwrap_err().to_string()).unwrap();
        let expected = native_read_case(&request, model).await;
        assert_eq!(observed["result"], expected, "{resize:?}, {vision:?}");
        let text = expected["content"][0]["text"].as_str().unwrap();
        assert_eq!(
            text.contains("Current model does not support images"),
            vision == Some(false)
        );
        assert_eq!(
            text.contains("original 2400x3, displayed at 2000x3"),
            bytes.len() > 3 && resize != Some(false)
        );
        assert_eq!(
            expected["content"].as_array().unwrap().len(),
            if bytes.len() == 3 && resize != Some(false) {
                1
            } else {
                2
            }
        );
    }
}

/// Supplies a completion gate through the real command-context owner, with no timer.
fn read_gate(
    runner: &maestro_extensions::ExtensionRunner<maestro_extensions::Local>,
) -> (
    tokio::sync::oneshot::Receiver<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    let (start, started) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let gate = std::cell::RefCell::new(Some((start, released)));
    runner.bind_command_context(Some(
        maestro_extensions::ExtensionCommandContextActions::new(
            maestro_extensions::CommandContextOperations {
                wait_for_idle: std::rc::Rc::new(move || {
                    let (start, released) = gate.borrow_mut().take().unwrap();
                    Box::pin(async move {
                        start.send(()).unwrap();
                        released.await?;
                        Ok(())
                    })
                }),
                ..crate::context_actions::silent_command_operations()
            },
        ),
    ));
    (started, release)
}

/// Parent completion precedes original content; native UTF-8 counts and cancellation survive wrapping.
#[test]
fn builtin_write_operations_create_parents_before_writing() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let runner = super::builtin_author::loaded().await;
        for executable in [false, true] {
            for (content, bytes) in [("", 0), ("a\0z", 3), ("é", 2), ("😀", 4), ("aé😀", 7)] {
                observe_write(
                    &runner,
                    json!({"content":content,"executable":executable}),
                    "ready",
                    None,
                )
                .await;
                assert_eq!(native_write_result(content).await, json!({"content":[{"type":"text","text":format!("Successfully wrote {bytes} bytes to parent/雪.txt")}]}));
            }
            observe_write_stages(&runner, executable).await;
        }
    });
}

/// Covers each reached effect without nesting the completion matrix in the content matrix.
async fn observe_write_stages(
    runner: &maestro_extensions::ExtensionRunner<maestro_extensions::Local>,
    executable: bool,
) {
    for stage in ["mkdir", "write"] {
        observe_write(
            runner,
            json!({"content":"é😀\0","executable":executable}),
            "pending",
            Some(stage),
        )
        .await;
        for mode in ["ready", "pending", "preabort", "pending-abort"] {
            observe_write(
                runner,
                json!({"content":"é😀\0","executable":executable,"fail":stage}),
                mode,
                Some(stage),
            )
            .await;
        }
    }
}

/// Runs ready and gated effects through the real compiled author and arbitrary caller signal.
async fn observe_write(
    runner: &maestro_extensions::ExtensionRunner<maestro_extensions::Local>,
    mut request: serde_json::Value,
    mode: &str,
    stage: Option<&str>,
) {
    let signal = read_signal(runner);
    request["gate"] = json!(mode.starts_with("pending").then_some(stage).flatten());
    request["signal"] = json!(match mode {
        "preabort" => Some("preaborted"),
        "pending-abort" => Some("relay"),
        _ => None,
    });
    let command = runner.get_command("bridge-observe").unwrap().command;
    let work = Box::pin((command.handler)(
        format!("builtin-custom-write {request}"),
        runner.create_command_context(),
    ));
    let result = if mode.starts_with("pending") {
        let (started, release) = read_gate(runner);
        let waiting = match futures_util::future::select(work, started).await {
            futures_util::future::Either::Left(_) => {
                panic!("write completed before the {stage:?} operation gate")
            }
            futures_util::future::Either::Right((started, waiting)) => {
                started.unwrap();
                waiting
            }
        };
        if mode == "pending-abort" {
            signal.abort();
        }
        release.send(()).unwrap();
        waiting.await
    } else {
        work.await
    };
    let observed: serde_json::Value =
        serde_json::from_str(&result.unwrap_err().to_string()).unwrap();
    let mut calls = vec![];
    if mode != "preabort" {
        calls.push(json!(["mkdir", "/virtual/parent"]));
        if request["fail"] != "mkdir" && !(mode == "pending-abort" && stage == Some("mkdir")) {
            calls.push(json!([
                "write",
                "/virtual/parent/雪.txt",
                request["content"],
                true
            ]));
        }
    }
    let mut expected = json!({"calls":calls});
    if mode.ends_with("abort") {
        expected["error"] = json!("Operation aborted");
        expected["notified"] = json!(u32::from(mode == "pending-abort"));
    } else if let Some(stage) = request["fail"].as_str() {
        expected["error"] = json!(format!("refused {stage} 雪"));
    } else {
        expected["result"] = native_write_result(request["content"].as_str().unwrap()).await;
    }
    assert_eq!(observed, expected, "{mode}: {request}");
}

/// Supplies no filesystem effects to the independent native result comparator.
struct WriteAnswer;
impl maestro_extensions::WriteOperations for WriteAnswer {
    fn mkdir(&self, _: String) -> ReadFuture<()> {
        Box::pin(std::future::ready(Ok(())))
    }
    fn write_file(&self, _: String, _: String) -> ReadFuture<()> {
        Box::pin(std::future::ready(Ok(())))
    }
}
/// Gets the complete result from the owning individual factory, not a guest output post-pass.
async fn native_write_result(content: &str) -> serde_json::Value {
    let tool = maestro_extensions::local::create_write_tool_definition::<()>(
        "/virtual".into(),
        maestro_extensions::WriteToolOptions {
            operations: Some(std::sync::Arc::new(WriteAnswer)),
        },
    );
    let tool = maestro_extensions::local::wrap_tool_definition(tool, None);
    let result = (tool.execute)(
        "native-write".into(),
        json!({"path":"parent/雪.txt","content":content}),
        None,
        None,
    )
    .await
    .unwrap();
    serde_json::to_value(result).unwrap()
}

/// Edit access consumes text/numeric codes before display; later failures keep their reached phase.
#[test]
fn builtin_operation_errors_keep_codes_and_messages() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let runner = super::builtin_author::loaded().await;
        for executable in [false, true] {
            for (stage, code, error) in [
                (
                    "access",
                    json!(null),
                    "Could not edit file: 雪.txt. refused access 雪.",
                ),
                (
                    "access",
                    json!("EACCES"),
                    "Could not edit file: 雪.txt. Error code: EACCES.",
                ),
                (
                    "access",
                    json!(17),
                    "Could not edit file: 雪.txt. Error code: 17.",
                ),
                ("read", json!("EACCES"), "refused read 雪"),
                ("write", json!(17), "refused write 雪"),
                ("", json!(null), ""),
            ] {
                observe_edit_error(&runner, executable, (stage, code, error)).await;
            }
        }
    });
}

/// Exercises both immediately-ready and positively observed pending callbacks.
async fn observe_edit_error(
    runner: &maestro_extensions::ExtensionRunner<maestro_extensions::Local>,
    executable: bool,
    case: (&str, serde_json::Value, &str),
) {
    let (stage, code, error) = case;
    for pending in [false, true] {
        let request = json!({"bytes":b"one\ntwo\n","args":{"path":"雪.txt","edits":[{"oldText":"one","newText":"new"}]},"executable":executable,"fail":(!stage.is_empty()).then_some(stage),"code":code,"gate":pending.then_some(if stage.is_empty() { "access" } else { stage })});
        let command = runner.get_command("bridge-observe").unwrap().command;
        let gate = pending.then(|| read_gate(runner));
        let work = Box::pin((command.handler)(
            format!("builtin-custom-edit {request}"),
            runner.create_command_context(),
        ));
        let result = if let Some((started, release)) = gate {
            let waiting = match futures_util::future::select(work, started).await {
                futures_util::future::Either::Left(_) => {
                    panic!("edit completed before its {stage} operation gate")
                }
                futures_util::future::Either::Right((started, waiting)) => {
                    started.unwrap();
                    waiting
                }
            };
            release.send(()).unwrap();
            waiting.await
        } else {
            work.await
        };
        let observed: serde_json::Value =
            serde_json::from_str(&result.unwrap_err().to_string()).unwrap();
        let mut calls = vec![json!(["access", "/virtual/雪.txt"])];
        if stage != "access" {
            calls.push(json!(["read", "/virtual/雪.txt"]));
        }
        if stage != "access" && stage != "read" {
            calls.push(json!(["write", "/virtual/雪.txt"]));
        }
        if stage.is_empty() {
            calls.push(json!(["written", "new\ntwo\n"]));
            assert_eq!(
                observed,
                json!({"calls":calls,"result":native_edit_success().await})
            );
        } else {
            assert_eq!(observed, json!({"calls":calls,"error":error}), "{request}");
        }
    }
}

/// Independent native effects for the complete successful edit result.
struct EditAnswer;
impl maestro_extensions::EditOperations for EditAnswer {
    fn access(&self, _: String) -> ReadFuture<()> {
        Box::pin(std::future::ready(Ok(())))
    }
    fn read_file(&self, _: String) -> ReadFuture<Vec<u8>> {
        Box::pin(std::future::ready(Ok(b"one\ntwo\n".to_vec())))
    }
    fn write_file(&self, _: String, content: String) -> ReadFuture<()> {
        assert_eq!(content, "new\ntwo\n");
        Box::pin(std::future::ready(Ok(())))
    }
}
/// Delegates the complete diff/result expectation to its actual native owner.
async fn native_edit_success() -> serde_json::Value {
    let definition = maestro_extensions::local::create_edit_tool_definition::<()>(
        "/virtual".into(),
        maestro_extensions::EditToolOptions {
            operations: Some(std::sync::Arc::new(EditAnswer)),
        },
    );
    let tool = maestro_extensions::local::wrap_tool_definition(definition, None);
    let result = (tool.execute)(
        "native-edit".into(),
        json!({"path":"雪.txt","edits":[{"oldText":"one","newText":"new"}]}),
        None,
        None,
    )
    .await
    .unwrap();
    serde_json::to_value(result).unwrap()
}

/// Reached edit effects observed through the original native event bus.
type EditCalls = std::rc::Rc<std::cell::RefCell<Vec<serde_json::Value>>>;

/// Preparation keeps native mutation separate from validation, execution and saved replay.
#[test]
fn builtin_edit_preparation_precedes_validation_and_replay() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let (runner, events) = super::builtin_author::loaded_with_events().await;
        let runner = std::rc::Rc::new(runner);
        let calls = EditCalls::default();
        let observed = calls.clone();
        let _subscription = events.on(
            "builtin-edit-effect",
            std::rc::Rc::new(move |data, _| {
                observed.borrow_mut().push((*data).clone());
                Ok(None)
            }),
        );
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("edit_preparation.json")).unwrap();
        assert_eq!(cases.len(), 22);
        for executable in [false, true] {
            let registered = register_edit(&runner, json!({"executable":executable})).await;
            for case in &cases {
                check_edit_preparation(&runner, &registered, &calls, case).await;
            }
            check_edit_prepare_failure(&runner, &calls, executable).await;
        }
    });
}

/// Obtains the actual registration that the compiled author admits through the facade.
async fn register_edit(
    runner: &maestro_extensions::ExtensionRunner<maestro_extensions::Local>,
    request: serde_json::Value,
) -> maestro_extensions::types::RegisteredTool<maestro_extensions::Local> {
    let command = runner.get_command("bridge-observe").unwrap().command;
    (command.handler)(
        format!("builtin-register-edit {request}"),
        runner.create_command_context(),
    )
    .await
    .unwrap();
    runner
        .get_all_registered_tools()
        .into_iter()
        .find(|tool| tool.definition.name() == "builtin-edit")
        .unwrap()
}

/// Validates returned arguments while history keeps exactly the source-recorded mutation.
async fn check_edit_preparation(
    runner: &std::rc::Rc<maestro_extensions::ExtensionRunner<maestro_extensions::Local>>,
    registered: &maestro_extensions::types::RegisteredTool<maestro_extensions::Local>,
    calls: &EditCalls,
    case: &serde_json::Value,
) {
    calls.borrow_mut().clear();
    let tool = maestro_extensions::wrap_registered_tool(registered, runner.clone());
    let mut retained = case["input"].clone();
    let prepared = (tool.prepare_arguments.as_ref().unwrap())(&mut retained).unwrap();
    let id = case["id"].as_str().unwrap();
    assert_eq!(prepared, case["expected"], "{id}");
    assert_eq!(retained, case["retained"], "{id}");
    assert!(
        calls.borrow().is_empty(),
        "preparation executed effects: {id}"
    );
    replay_edit_arguments(&retained, &case["retained"]);
    let call = maestro_models::ToolCall {
        id: "edit".into(),
        name: "builtin-edit".into(),
        arguments: prepared,
        thought_signature: None,
    };
    let checked = maestro_models::validate_tool_arguments(&tool.definition, &call);
    assert_eq!(checked.is_ok(), edit_schema_valid(id), "{id}: {checked:?}");
    let Ok(arguments) = checked else {
        assert!(
            calls.borrow().is_empty(),
            "schema failure executed effects: {id}"
        );
        return;
    };
    let result = (tool.execute)("edit".into(), arguments, None, None).await;
    let written = prepared_edit_content(id);
    assert_eq!(result.is_ok(), written.is_some(), "{id}: {result:?}");
    if let Some(written) = written {
        assert_eq!(
            *calls.borrow(),
            vec![
                json!(["access", "/virtual/f"]),
                json!(["read", "/virtual/f"]),
                json!(["write", "/virtual/f", written])
            ],
            "{id}"
        );
    } else {
        assert!(
            !calls.borrow().iter().any(|call| call[0] == "write"),
            "{id}"
        );
    }
}

/// The corpus's complete schemas admit these argument classes, including empty edit lists.
fn edit_schema_valid(id: &str) -> bool {
    matches!(
        id,
        "string_edits"
            | "legacy"
            | "mixed"
            | "empty"
            | "prepare-array_string_edits"
            | "prepare-legacy_empty"
            | "prepare-mixed_string_legacy"
    )
}

/// Literal results from replacing the independent original a/c lines.
fn prepared_edit_content(id: &str) -> Option<&'static str> {
    match id {
        "string_edits" | "legacy" => Some("b\nc\n"),
        "mixed" | "prepare-mixed_string_legacy" => Some("b\nd\n"),
        _ => None,
    }
}

/// A callback failure retains preceding in-place mutations without executing any effect.
async fn check_edit_prepare_failure(
    runner: &std::rc::Rc<maestro_extensions::ExtensionRunner<maestro_extensions::Local>>,
    calls: &EditCalls,
    executable: bool,
) {
    calls.borrow_mut().clear();
    let registered = register_edit(
        runner,
        json!({"executable":executable,"prepareFailure":true}),
    )
    .await;
    let tool = maestro_extensions::wrap_registered_tool(&registered, runner.clone());
    let mut retained = json!({"path":"f","edits":"[{\"oldText\":\"a\",\"newText\":\"b\"}]"});
    let error = (tool.prepare_arguments.as_ref().unwrap())(&mut retained).unwrap_err();
    assert_eq!(error.to_string(), "authored preparation failed");
    let expected = json!({"path":"f","edits":[{"oldText":"a","newText":"b"}]});
    assert_eq!(retained, expected);
    assert!(calls.borrow().is_empty());
    replay_edit_arguments(&retained, &expected);
}

/// Saves the retained assistant call, then projects the session into the next model request.
fn replay_edit_arguments(retained: &serde_json::Value, expected: &serde_json::Value) {
    let mut session = maestro_session::SessionManager::in_memory(Some("/edit-replay")).unwrap();
    session.append_message(json!({"role":"assistant","content":[{"type":"toolCall","id":"edit","name":"builtin-edit","arguments":retained}],"api":"fixture","provider":"fixture","model":"fixture","stopReason":"toolUse","timestamp":1,"usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}}})).unwrap();
    let history = session.build_session_context().unwrap();
    assert_eq!(history.messages[0]["content"][0]["arguments"], *expected);
    let messages: Vec<maestro_models::Message> = history
        .messages
        .into_iter()
        .map(|message| serde_json::from_value(message).unwrap())
        .collect();
    let model = super::model_support::model("fixture");
    let request = maestro_models::transform_messages(&messages, &model.read().unwrap(), None);
    let maestro_models::Message::Assistant(assistant) = &request[0] else {
        panic!("assistant replay absent")
    };
    let maestro_models::AssistantContent::ToolCall(call) = &assistant.content[0] else {
        panic!("edit replay absent")
    };
    assert_eq!(call.arguments, *expected);
}

/// Matching uses the original snapshot; failed matches never write and BOM/CRLF survive.
#[test]
fn builtin_edit_callbacks_preserve_snapshot_and_file_bytes() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let runner = super::builtin_author::loaded().await;
        for executable in [false, true] {
            for (source, edits, written) in [
                ("one\ntwo\nthree\n",json!([{"oldText":"one","newText":"two"},{"oldText":"two","newText":"TWO"}]),Some("two\nTWO\nthree\n")),
                ("one\ntwo\n",json!([{"oldText":"one\ntwo","newText":"new"},{"oldText":"two","newText":"TWO"}]),None),
                ("one\ntwo\n",json!([{"oldText":"one","newText":"ONE"},{"oldText":"missing","newText":"new"}]),None),
                ("one\none\n",json!([{"oldText":"one","newText":"ONE"}]),None),
                ("\u{feff}one\r\ntwo\r\n",json!([{"oldText":"one","newText":"new\nsecond"}]),Some("\u{feff}new\r\nsecond\r\ntwo\r\n")),
                ("first\nsecond\n",json!([{"oldText":"second","newText":"SECOND"}]),Some("first\nSECOND\n")),
            ] {
                observe_snapshot_edit(&runner, executable, (source, edits, written)).await;
            }
        }
    });
}

/// Observes ordered callbacks and complete native result/error records for one snapshot.
async fn observe_snapshot_edit(
    runner: &maestro_extensions::ExtensionRunner<maestro_extensions::Local>,
    executable: bool,
    (source, edits, written): (&str, serde_json::Value, Option<&str>),
) {
    let command = runner.get_command("bridge-observe").unwrap().command;
    let args = json!({"path":"雪.txt","edits":edits});
    let request = json!({"bytes":source.as_bytes(),"args":args,"executable":executable});
    let result = (command.handler)(
        format!("builtin-custom-edit {request}"),
        runner.create_command_context(),
    )
    .await;
    let observed: serde_json::Value =
        serde_json::from_str(&result.unwrap_err().to_string()).unwrap();
    let mut calls = vec![
        json!(["access", "/virtual/雪.txt"]),
        json!(["read", "/virtual/雪.txt"]),
    ];
    if let Some(written) = written {
        calls.push(json!(["write", "/virtual/雪.txt"]));
        calls.push(json!(["written", written]));
    }
    let mut expected = json!({"calls":calls});
    match native_snapshot_edit(source, args, written).await {
        Ok(result) => {
            assert_eq!(
                result["details"]["firstChangedLine"],
                if source.starts_with("first") { 2 } else { 1 }
            );
            expected["result"] = result;
        }
        Err(error) => expected["error"] = json!(error),
    }
    assert_eq!(observed, expected, "{request}");
}

/// Independent literal file bytes and the sole permitted successful replacement.
struct SnapshotEdit {
    source: Vec<u8>,
    written: Option<String>,
}
impl maestro_extensions::EditOperations for SnapshotEdit {
    fn access(&self, _: String) -> ReadFuture<()> {
        Box::pin(std::future::ready(Ok(())))
    }
    fn read_file(&self, _: String) -> ReadFuture<Vec<u8>> {
        Box::pin(std::future::ready(Ok(self.source.clone())))
    }
    fn write_file(&self, _: String, content: String) -> ReadFuture<()> {
        assert_eq!(Some(content), self.written, "unexpected native write");
        Box::pin(std::future::ready(Ok(())))
    }
}

/// Compares the complete native diff and error rather than duplicating message templates.
async fn native_snapshot_edit(
    source: &str,
    args: serde_json::Value,
    written: Option<&str>,
) -> Result<serde_json::Value, String> {
    let definition = maestro_extensions::local::create_edit_tool_definition::<()>(
        "/virtual".into(),
        maestro_extensions::EditToolOptions {
            operations: Some(std::sync::Arc::new(SnapshotEdit {
                source: source.as_bytes().to_vec(),
                written: written.map(str::to_owned),
            })),
        },
    );
    let tool = maestro_extensions::local::wrap_tool_definition(definition, None);
    let result = (tool.execute)("native-snapshot".into(), args, None, None)
        .await
        .map_err(|error| error.to_string())?;
    Ok(serde_json::to_value(result).unwrap())
}
