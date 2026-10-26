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

/// A failing replacement stops the native read before any later observation.
#[test]
fn builtin_read_operation_failures_stop_at_the_reached_stage() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let runner = super::builtin_author::loaded().await;
        let command = runner.get_command("bridge-observe").unwrap().command;
        for (failure, stages) in [
            ("access", vec!["access"]),
            ("mime", vec!["access", "mime"]),
            ("read", vec!["access", "mime", "read"]),
        ] {
            let request = json!({"detect":true,"mime":null,"bytes":[],"fail":failure});
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
                json!({"error":format!("refused {failure} 雪"),"calls":calls,"prepares":false})
            );
        }
    });
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
    let options = maestro_extensions::ReadToolOptions {
        auto_resize_images: request["resize"].as_bool(),
        operations: Some(std::sync::Arc::new(ReadCase {
            bytes: serde_json::from_value(request["bytes"].clone()).unwrap(),
            mime: request["mime"].as_str().map(str::to_owned),
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
    let result = (tool.execute)("comparator".into(), json!({"path":"雪.bin"}), None, None)
        .await
        .unwrap();
    serde_json::to_value(result).unwrap()
}

/// Native callback responses preserve the exact original bytes.
struct ReadCase {
    bytes: Vec<u8>,
    mime: Option<String>,
}
impl maestro_extensions::ReadOperations for ReadCase {
    fn access(&self, _: String) -> ReadFuture<()> {
        Box::pin(std::future::ready(Ok(())))
    }
    fn read_file(&self, _: String) -> ReadFuture<Vec<u8>> {
        Box::pin(std::future::ready(Ok(self.bytes.clone())))
    }
    fn detect_image_mime_type(&self, _: String) -> ReadFuture<Option<String>> {
        Box::pin(std::future::ready(Ok(self.mime.clone())))
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
