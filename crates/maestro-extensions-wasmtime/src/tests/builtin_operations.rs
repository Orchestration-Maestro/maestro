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
