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

/// The compiled author forwards glob operands and native result/error selection unchanged.
#[test]
fn builtin_find_callbacks_receive_pattern_root_ignore_and_limit() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let runner = super::builtin_author::loaded().await;
        for executable in [false, true] {
            find_cases(&runner, executable).await;
        }
    });
}

/// Runs the supplied limit, path, result and failure domains on one execution projection.
async fn find_cases(
    runner: &maestro_extensions::ExtensionRunner<maestro_extensions::Local>,
    executable: bool,
) {
    for limit in [
        None,
        Some(json!(null)),
        Some(json!(0)),
        Some(json!(2)),
        Some(json!(1000)),
    ] {
        let mut args = json!({"pattern":"**/雪*.rs","path":"雪"});
        if let Some(limit) = limit {
            args["limit"] = limit;
        }
        for paths in [
            json!([]),
            json!(["b.rs", "/virtual/雪/a.rs", "b.rs"]),
            json!(["x".repeat(52000)]),
        ] {
            let request = json!({"args":args,"exists":true,"paths":paths,"executable":executable});
            observe_find(runner, request).await;
        }
    }
    for args in [
        json!({"pattern":"*.rs"}),
        json!({"pattern":"*.rs","path":""}),
    ] {
        observe_find(
            runner,
            json!({"args":args,"exists":true,"paths":["a.rs"],"executable":executable}),
        )
        .await;
    }
    observe_find(runner, json!({"args":{"pattern":"*","path":"雪"},"exists":false,"paths":[],"executable":executable})).await;
    for stage in ["exists", "glob"] {
        for pending in [false, true] {
            observe_find(
                runner,
                json!({"args":{"pattern":"*","path":"雪"},"exists":true,"paths":[],
                        "fail":stage,"gate":pending.then_some(stage),"executable":executable}),
            )
            .await;
        }
    }
}

/// Records reached effects only after the owning call and any gated callback have completed.
async fn observe_find(
    runner: &maestro_extensions::ExtensionRunner<maestro_extensions::Local>,
    request: serde_json::Value,
) {
    let command = runner.get_command("bridge-observe").unwrap().command;
    let waiting = Box::pin((command.handler)(
        format!("builtin-custom-find {request}"),
        runner.create_command_context(),
    ));
    let result = if request["gate"].is_string() {
        let (started, release) = read_gate(runner);
        let waiting = match futures_util::future::select(waiting, started).await {
            futures_util::future::Either::Left(_) => {
                panic!("find completed before the operation gate")
            }
            futures_util::future::Either::Right((started, waiting)) => {
                started.unwrap();
                waiting
            }
        };
        release.send(()).unwrap();
        waiting.await
    } else {
        waiting.await
    };
    let observed: serde_json::Value =
        serde_json::from_str(&result.unwrap_err().to_string()).unwrap();
    let root = if request["args"]["path"] == "雪" {
        "/virtual/雪"
    } else {
        "/virtual"
    };
    let mut calls = vec![json!(["exists", root])];
    if request["exists"] == true && request["fail"] != "exists" {
        calls.push(json!(["glob",request["args"]["pattern"],root,
            {"ignore":["**/node_modules/**","**/.git/**"],"limit":request["args"]["limit"].as_f64().unwrap_or(1000.0)}]));
    }
    let expected = native_find(&request).await;
    assert_eq!(observed["calls"], json!(calls));
    match expected {
        Ok(result) => assert_eq!(observed, json!({"calls":calls,"result":result})),
        Err(error) => assert_eq!(observed, json!({"calls":calls,"error":error})),
    }
}

/// Independent native comparator, without any guest result or operand conversion.
struct FindAnswer(serde_json::Value);
impl maestro_extensions::FindOperations for FindAnswer {
    fn exists(&self, _: String) -> ReadFuture<bool> {
        let result = self
            .answer("exists")
            .map(|()| self.0["exists"].as_bool().unwrap());
        Box::pin(std::future::ready(result))
    }
    fn glob(
        &self,
        _: String,
        _: String,
        _: maestro_extensions::FindGlobOptions,
    ) -> ReadFuture<Vec<String>> {
        let result = self
            .answer("glob")
            .map(|()| serde_json::from_value(self.0["paths"].clone()).unwrap());
        Box::pin(std::future::ready(result))
    }
}
impl FindAnswer {
    /// Supplies the same authored error without reusing the reverse adapter.
    fn answer(&self, stage: &str) -> Result<(), maestro_extensions::DiagnosticErrorInfo> {
        if self.0["fail"] == stage {
            Err(maestro_extensions::DiagnosticErrorInfo {
                name: Some("FindFailure".into()),
                message: format!("refused {stage} 雪"),
                stack: None,
                code: Some(maestro_extensions::DiagnosticCode::Number(17.0)),
            })
        } else {
            Ok(())
        }
    }
}
/// Runs the individual native owner as the comparison boundary for delegated messages.
async fn native_find(request: &serde_json::Value) -> Result<serde_json::Value, String> {
    let definition = maestro_extensions::local::create_find_tool_definition::<()>(
        "/virtual".into(),
        maestro_extensions::FindToolOptions {
            operations: Some(std::sync::Arc::new(FindAnswer(request.clone()))),
        },
        std::sync::Arc::new(maestro_extensions::ToolsManager::new(
            "/unused".into(),
            None,
        )),
    );
    let tool = maestro_extensions::local::wrap_tool_definition(definition, None);
    let result = (tool.execute)("native-find".into(), request["args"].clone(), None, None)
        .await
        .map_err(|error| error.to_string())?;
    Ok(serde_json::to_value(result).unwrap())
}

/// Real ripgrep matches remain native while ready and pending callbacks supply context.
#[test]
fn builtin_grep_callbacks_preserve_native_search_and_context() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let file = scratch.path().join("雪.txt");
        std::fs::write(&file, "before\nneedle\nafter\n").unwrap();
        let manager = std::sync::Arc::new(
            maestro_extensions::ToolsManager::new(scratch.path().display().to_string(), None)
                .with_offline(Some(true)),
        );
        let runner = super::builtin_author::loaded_with_tool_services(
            maestro_extensions::ToolConstruction {
                tools_manager: manager.clone(),
                bin_dir: scratch.path().display().to_string(),
            },
        )
        .await
        .0;
        for executable in [false, true] {
            for request in grep_cases(scratch.path(), &file, executable) {
                observe_grep(&runner, &manager, request).await;
            }
        }
    });
}

/// Supplies actual file/directory roots and independent authored context text.
fn grep_cases(
    root: &std::path::Path,
    file: &std::path::Path,
    executable: bool,
) -> Vec<serde_json::Value> {
    let mut cases = Vec::new();
    for directory in [false, true] {
        for context in [0, 1] {
            let args = json!({"pattern":"needle","path":if directory {root} else {file},"context":context});
            let base = json!({"cwd":root,"args":args,"custom":true,"directory":directory,"text":"authored before\nauthored needle\nauthored after\n","executable":executable});
            cases.extend(grep_outcomes(&base));
        }
    }
    cases
}
/// Ready and pending callbacks exercise only phases the native owner reaches.
fn grep_outcomes(base: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut cases = vec![base.clone()];
    for stage in ["directory", "read"] {
        if stage == "read" && base["args"]["context"] == 0 {
            continue;
        }
        for pending in [false, true] {
            let mut request = base.clone();
            request["fail"] = json!(stage);
            request["gate"] = json!(pending.then_some(stage));
            cases.push(request);
        }
    }
    cases
}

/// Uses the real operation completion witness instead of timing a pending callback.
async fn observe_grep(
    runner: &maestro_extensions::ExtensionRunner<maestro_extensions::Local>,
    manager: &std::sync::Arc<maestro_extensions::ToolsManager>,
    request: serde_json::Value,
) {
    let command = runner.get_command("bridge-observe").unwrap().command;
    let waiting = Box::pin((command.handler)(
        format!("builtin-custom-grep {request}"),
        runner.create_command_context(),
    ));
    let result = if request["gate"].is_string() {
        let (started, release) = read_gate(runner);
        let waiting = match futures_util::future::select(waiting, started).await {
            futures_util::future::Either::Left(_) => {
                panic!("grep completed before the operation gate")
            }
            futures_util::future::Either::Right((started, waiting)) => {
                started.unwrap();
                waiting
            }
        };
        release.send(()).unwrap();
        waiting.await
    } else {
        waiting.await
    };
    let observed: serde_json::Value =
        serde_json::from_str(&result.unwrap_err().to_string()).unwrap();
    let mut calls = vec![json!([
        "directory",
        request["args"]["path"],
        "/builtin-caller"
    ])];
    if request["args"]["context"] == 1 && request["fail"] != "directory" {
        let path = std::path::Path::new(request["cwd"].as_str().unwrap()).join("雪.txt");
        calls.push(json!(["read", path, "/builtin-caller"]));
    }
    let definition = maestro_extensions::local::create_grep_tool_definition::<()>(
        request["cwd"].as_str().unwrap().into(),
        maestro_extensions::GrepToolOptions {
            operations: Some(std::sync::Arc::new(GrepAnswer(request.clone()))),
        },
        manager.clone(),
    );
    let tool = maestro_extensions::local::wrap_tool_definition(definition, None);
    let expected = (tool.execute)("native-grep".into(), request["args"].clone(), None, None).await;
    match expected {
        Ok(result) => assert_eq!(
            observed,
            json!({"calls":calls,"result":serde_json::to_value(result).unwrap()})
        ),
        Err(error) => assert_eq!(observed, json!({"calls":calls,"error":error.to_string()})),
    }
}
/// Independent native callback values used only by the comparator.
struct GrepAnswer(serde_json::Value);
impl maestro_extensions::GrepOperations for GrepAnswer {
    fn is_directory(&self, _: String) -> ReadFuture<bool> {
        let result = if self.0["fail"] == "directory" {
            Err(maestro_extensions::DiagnosticErrorInfo::error(
                "refused directory 雪",
            ))
        } else {
            Ok(self.0["directory"].as_bool().unwrap())
        };
        Box::pin(std::future::ready(result))
    }
    fn read_file(&self, _: String) -> ReadFuture<String> {
        let result = if self.0["fail"] == "read" {
            Err(maestro_extensions::DiagnosticErrorInfo::error(
                "refused read 雪",
            ))
        } else {
            Ok(self.0["text"].as_str().unwrap().into())
        };
        Box::pin(std::future::ready(result))
    }
}

/// A broken managed program, then the real ripgrep, proves the supplied manager is consumed.
#[test]
fn builtin_grep_uses_application_tool_services() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime,async {
        let scratch=maestro_test_directory::Scratch::new().unwrap();
        let bin=scratch.path().join("app-bin");
        std::fs::create_dir(&bin).unwrap();
        let binary=format!("rg{}",std::env::consts::EXE_SUFFIX);
        let source=std::env::split_paths(&std::env::var_os("PATH").unwrap()).map(|dir|dir.join(&binary)).find(|path|path.is_file()).expect("ripgrep is installed for native search tests");
        let installed=bin.join(binary);
        std::fs::write(&installed,"not an executable").unwrap();
        let file=scratch.path().join("雪.txt");
        std::fs::write(&file,"before\nneedle\nafter\n").unwrap();
        let manager=std::sync::Arc::new(maestro_extensions::ToolsManager::new(bin.display().to_string(),None).with_offline(Some(true)));
        assert_eq!(manager.get_tool_path(maestro_extensions::SearchTool::Rg).await,Some(installed.display().to_string()));
        let runner=super::builtin_author::loaded_with_tool_services(maestro_extensions::ToolConstruction {
            tools_manager:manager.clone(),bin_dir:bin.display().to_string(),
        }).await.0;
        let command=runner.get_command("bridge-observe").unwrap().command;
        for valid in [false,true] {
            if valid {std::fs::copy(&source,&installed).unwrap();}
            for executable in [false,true] {
                let request=json!({"cwd":scratch.path().to_str(),"args":{"pattern":"needle","path":file},"custom":false,"directory":false,"text":"","executable":executable});
                let response=(command.handler)(format!("builtin-custom-grep {request}"),runner.create_command_context()).await;
                let observed:serde_json::Value=serde_json::from_str(&response.unwrap_err().to_string()).unwrap();
                let tool=maestro_extensions::local::wrap_tool_definition(maestro_extensions::local::create_grep_tool_definition::<()>(scratch.path().display().to_string(),maestro_extensions::GrepToolOptions::default(),manager.clone()),None);
                let expected=(tool.execute)("native-managed".into(),request["args"].clone(),None,None).await;
                assert_managed_grep(&observed, expected, valid);
            }
        }
    });
}

/// Checks complete delegated failure records and the successful native search text.
fn assert_managed_grep(
    observed: &serde_json::Value,
    expected: Result<
        maestro_extensions::AgentToolResult<serde_json::Value>,
        maestro_extensions::DiagnosticErrorInfo,
    >,
    valid: bool,
) {
    if valid {
        assert_eq!(
            *observed,
            json!({"calls":[],"result":serde_json::to_value(expected.unwrap()).unwrap()})
        );
        assert_eq!(
            observed["result"],
            json!({"content":[{"type":"text","text":"雪.txt:2: needle"}]})
        );
    } else {
        assert_eq!(
            *observed,
            json!({"calls":[],"error":expected.unwrap_err().to_string()})
        );
        assert!(
            observed["error"]
                .as_str()
                .unwrap()
                .starts_with("Failed to run ripgrep:")
        );
    }
}

/// Listing policy stays native across both guest projections and ready/pending observations.
#[test]
fn builtin_listing_callbacks_keep_sort_suffix_and_skip_rules() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let runner = super::builtin_author::loaded().await;
        for executable in [false, true] {
            listing_cases(&runner, executable).await;
        }
    });
}

/// Supplies literal effect order independently of the native comparator's formatting.
async fn listing_cases(
    runner: &maestro_extensions::ExtensionRunner<maestro_extensions::Local>,
    executable: bool,
) {
    let base = json!({"args":{"path":"雪"},"exists":true,"directory":true,"names":["雪","skip","Dir","b","A"],"executable":executable});
    for (limit, reached) in [
        (None, vec!["A", "b", "Dir", "skip", "雪"]),
        (Some(json!(null)), vec!["A", "b", "Dir", "skip", "雪"]),
        (Some(json!(0)), vec![]),
        (Some(json!(2)), vec!["A", "b"]),
    ] {
        let mut request = base.clone();
        if let Some(limit) = limit {
            request["args"]["limit"] = limit;
        }
        observe_listing(runner, request, &reached).await;
    }
    let mut empty = base.clone();
    empty["names"] = json!([]);
    observe_listing(runner, empty, &[]).await;
    let mut large = base.clone();
    let name = "x".repeat(52000);
    large["names"] = json!([name]);
    observe_listing(runner, large, &[&name]).await;
    for (key, value) in [("exists", false), ("directory", false)] {
        let mut request = base.clone();
        request[key] = json!(value);
        observe_listing(runner, request, &[]).await;
    }
    for stage in ["exists", "stat", "readdir", "skip"] {
        for pending in [false, true] {
            let mut request = base.clone();
            if stage != "skip" {
                request["fail"] = json!(stage);
            }
            request["gate"] = json!(pending.then_some(stage));
            observe_listing(
                runner,
                request,
                if stage == "skip" {
                    &["A", "b", "Dir", "skip", "雪"]
                } else {
                    &[]
                },
            )
            .await;
        }
    }
    for stage in ["exists", "stat", "readdir"] {
        let mut request = base.clone();
        request["gate"] = json!(stage);
        observe_listing(runner, request, &["A", "b", "Dir", "skip", "雪"]).await;
    }
}

/// Waits for an actual callback completion witness, never an elapsed-time guess.
async fn observe_listing(
    runner: &maestro_extensions::ExtensionRunner<maestro_extensions::Local>,
    request: serde_json::Value,
    entries: &[&str],
) {
    let command = runner.get_command("bridge-observe").unwrap().command;
    let work = Box::pin((command.handler)(
        format!("builtin-custom-ls {request}"),
        runner.create_command_context(),
    ));
    let result = if request["gate"].is_string() {
        let (started, release) = read_gate(runner);
        let waiting = match futures_util::future::select(work, started).await {
            futures_util::future::Either::Left(_) => {
                panic!("listing completed before its operation gate")
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
    let mut calls = vec![json!(["exists", "/virtual/雪"])];
    if request["exists"] == true && request["fail"] != "exists" {
        calls.push(json!(["stat", "/virtual/雪"]));
    }
    if request["exists"] == true
        && request["directory"] == true
        && request["fail"] != "exists"
        && request["fail"] != "stat"
    {
        calls.push(json!(["readdir", "/virtual/雪"]));
    }
    calls.extend(
        entries
            .iter()
            .map(|name| json!(["stat", format!("/virtual/雪/{name}")])),
    );
    match native_listing(&request).await {
        Ok(result) => {
            assert_eq!(observed, json!({"calls":calls,"result":result}));
            if entries == ["A", "b", "Dir", "skip", "雪"] {
                assert_eq!(
                    observed["result"],
                    json!({"content":[{"type":"text","text":"A\nb\nDir/\n雪"}]})
                );
            }
        }
        Err(error) => assert_eq!(observed, json!({"calls":calls,"error":error})),
    }
}

/// Independent native callback responses with the same authored failure fields.
struct ListingAnswer(serde_json::Value);
impl ListingAnswer {
    /// Leaves root formatting, per-entry skipping and diagnostics to the native owner.
    fn answer<T>(
        &self,
        stage: &str,
        value: T,
    ) -> Result<T, maestro_extensions::DiagnosticErrorInfo> {
        if self.0["fail"] == stage || stage == "skip" {
            Err(maestro_extensions::DiagnosticErrorInfo {
                name: Some("ListingFailure".into()),
                message: format!("refused {stage} 雪"),
                stack: None,
                code: Some(maestro_extensions::DiagnosticCode::Number(17.0)),
            })
        } else {
            Ok(value)
        }
    }
}
impl maestro_extensions::LsOperations for ListingAnswer {
    fn exists(&self, _: String) -> ReadFuture<bool> {
        Box::pin(std::future::ready(
            self.answer("exists", self.0["exists"].as_bool().unwrap()),
        ))
    }
    fn stat(&self, path: String) -> ReadFuture<bool> {
        let (stage, value) = if path == "/virtual/雪" {
            ("stat", self.0["directory"].as_bool().unwrap())
        } else if path.ends_with("/skip") {
            ("skip", false)
        } else {
            ("entry", path.ends_with("/Dir"))
        };
        Box::pin(std::future::ready(self.answer(stage, value)))
    }
    fn readdir(&self, _: String) -> ReadFuture<Vec<String>> {
        Box::pin(std::future::ready(self.answer(
            "readdir",
            serde_json::from_value(self.0["names"].clone()).unwrap(),
        )))
    }
}
/// Gets complete native result/error records without copying output templates into the facade.
async fn native_listing(request: &serde_json::Value) -> Result<serde_json::Value, String> {
    let definition = maestro_extensions::local::create_ls_tool_definition::<()>(
        "/virtual".into(),
        maestro_extensions::LsToolOptions {
            operations: Some(std::sync::Arc::new(ListingAnswer(request.clone()))),
        },
    );
    let tool = maestro_extensions::local::wrap_tool_definition(definition, None);
    let result = (tool.execute)("native-ls".into(), request["args"].clone(), None, None)
        .await
        .map_err(|error| error.to_string())?;
    Ok(serde_json::to_value(result).unwrap())
}
