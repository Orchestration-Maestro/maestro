//! Authored native file operations without a guest publication algorithm.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{CommandOptions, ExtensionAPI, ExtensionResult, io::NativeIo};
use serde_json::{Value, json};
use std::rc::Rc;

/// Registers the real author's byte-save and permission interface.
pub(super) fn register(api: &ExtensionAPI, io: Rc<dyn NativeIo>) -> ExtensionResult<()> {
    api.register_command(
        "native-files",
        CommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(move |args, context| {
                let io = io.clone();
                Box::pin(async move {
                    let reply = match execute(&*io, &args) {
                        Ok(value) => json!({"value": value}),
                        Err(error) => json!({"error": error}),
                    };
                    context.ui()?.set_title(&format!("NATIVE-IO:{reply}"))
                })
            }),
        },
    )
}

/// Sends literal operands to the retained host capability.
fn execute(io: &dyn NativeIo, args: &str) -> ExtensionResult<Value> {
    let args: Value = serde_json::from_str(args).map_err(|error| error.to_string())?;
    let path = args["path"].as_str().ok_or("path required")?;
    let mode =
        || serde_json::from_value::<u32>(args["mode"].clone()).map_err(|error| error.to_string());
    match args["op"].as_str() {
        Some("save") => {
            let bytes: Vec<u8> =
                serde_json::from_value(args["bytes"].clone()).map_err(|error| error.to_string())?;
            Ok(json!(io.save_file(path, &bytes, mode()?)?))
        }
        Some("mode") => Ok(json!(io.file_mode(path)?)),
        Some("chmod") => io.set_file_mode(path, mode()?).map(|()| Value::Null),
        Some("mkdir") => io.create_dir(path, mode()?).map(|()| Value::Null),
        _ => Err("unknown native file operation".into()),
    }
}
