//! Author component exercising native local shell construction and returned operations.
// maestro-component: MAESTRO_LOCAL_SHELL_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

/// Construction, raw execution and interactive interception through the public facade.
#[cfg(target_arch = "wasm32")]
mod author {
    use maestro_extensions_wasm::{
        BashExecOptions, CommandOptions, ExtensionAPI, ExtensionCommandContext,
        ExtensionEventResult, ExtensionFuture, ExtensionResult, Presence, UserBashEventResult,
    };
    use serde::Deserialize;
    use std::{cell::RefCell, rc::Rc};

    /// Selects raw execution or the native tool's output policy.
    #[derive(Clone, Copy, Default, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Mode {
        /// Raw local execution.
        #[default]
        Raw,
        /// Existing native tool formatting and artifact ownership.
        Tool,
    }

    /// Literal execution inputs for the native shell.
    #[derive(Deserialize)]
    struct Request {
        /// Command passed unchanged to the operation.
        command: String,
        /// Original execution directory.
        cwd: String,
        /// Optional executable selected without probing it at construction.
        shell: Option<String>,
        /// Optional seconds forwarded to the native driver.
        timeout: Option<f64>,
        /// Complete environment when supplied, including unset entries.
        env: Option<indexmap::IndexMap<String, Option<String>>>,
        /// Executes the same retained backend through the native tool's output owner.
        #[serde(default)]
        mode: Mode,
        /// Uses the command context's independently supplied signal.
        #[serde(default)]
        signal: bool,
        /// Aborts through the context when the native output arrives.
        #[serde(default)]
        abort: bool,
        /// Rejects raw output observations without rejecting execution.
        #[serde(default)]
        reject_output: bool,
    }

    /// Registers both direct execution and a real interactive shell interception.
    pub(super) fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        Box::pin(async move { register(&api) })
    }

    /// Retains local operations through both command and event registrations.
    fn register(api: &ExtensionAPI) -> ExtensionResult<()> {
        api.on(
            "session_start",
            Rc::new(|_, context| {
                Box::pin(async move {
                    context.ui()?.set_title("LOCAL-SHELL-READY")?;
                    Ok(None)
                })
            }),
        )?;
        let factory = api.clone();
        api.on(
            "user_bash",
            Rc::new(move |_, _| {
                let operations = factory.create_local_bash_operations(Some("/bin/bash".into()));
                Box::pin(async move {
                    Ok(Some(ExtensionEventResult::UserBash(UserBashEventResult {
                        operations: Some(operations?),
                        result: Presence::Missing,
                    })))
                })
            }),
        )?;
        let factory = api.clone();
        api.register_command(
            "local-shell",
            CommandOptions {
                description: None,
                get_argument_completions: None,
                handler: Rc::new(move |args, context| {
                    let api = factory.clone();
                    Box::pin(async move { execute(&api, &context, &args).await })
                }),
            },
        )
    }

    /// Returns the raw bytes and native outcome, without tool or user-shell formatting.
    async fn execute(
        api: &ExtensionAPI,
        context: &ExtensionCommandContext,
        args: &str,
    ) -> ExtensionResult<()> {
        let request: Request = serde_json::from_str(args).map_err(|error| error.to_string())?;
        let operations = api.create_local_bash_operations(request.shell.clone())?;
        if matches!(request.mode, Mode::Tool) {
            return execute_tool(api, context, request, operations).await;
        }
        let bytes = Rc::new(RefCell::new(Vec::new()));
        let collected = bytes.clone();
        let context = context.clone();
        let signal = if request.signal {
            context.signal()?
        } else {
            None
        };
        let options = BashExecOptions {
            signal,
            timeout: request.timeout,
            env: request.env,
            on_data: Rc::new(move |chunk| {
                collected.borrow_mut().extend_from_slice(chunk);
                if request.abort {
                    context.abort()?;
                }
                if request.reject_output {
                    return Err("raw observer rejected é".into());
                }
                Ok(())
            }),
        };
        let result = (operations.exec)(request.command, request.cwd, options).await;
        let result = result.map(|result| result.exit_code);
        Err(serde_json::json!({"bytes":*bytes.borrow(),"result":result}).to_string())
    }

    /// Uses the native output formatter and artifact owner around the retained local backend.
    async fn execute_tool(
        api: &ExtensionAPI,
        context: &ExtensionCommandContext,
        request: Request,
        operations: maestro_extensions_wasm::BashOperations,
    ) -> ExtensionResult<()> {
        let signal = if request.signal {
            context.signal()?
        } else {
            None
        };
        let context = context.clone();
        let update: maestro_extensions_wasm::AgentToolUpdateCallback = Rc::new(move |partial| {
            let value = serde_json::to_value(partial).map_err(|error| error.to_string())?;
            let ready = value["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|block| block["text"].as_str())
                .any(|text| text.ends_with("ready"));
            if request.abort && ready {
                context.abort()?;
            }
            Ok(())
        });
        let tool = api.create_bash_tool(
            request.cwd,
            maestro_extensions_wasm::BashToolOptions {
                operations: Some(operations),
                ..Default::default()
            },
        )?;
        let args = serde_json::json!({"command":request.command,"timeout":request.timeout});
        let result = (tool.execute)("local".into(), args, signal, Some(update)).await;
        Err(serde_json::json!({"tool":result}).to_string())
    }
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(author::factory);
