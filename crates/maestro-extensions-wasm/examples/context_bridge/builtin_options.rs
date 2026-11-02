//! Executes selected factory options at their native consuming operations.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    BashExecResult, BashOperations, ExtensionAPI, ExtensionCommandContext, ExtensionResult,
    OperationFuture, ReadOperations, Tool, ToolDefinition, ToolsOptions,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Original operands observed by replacement operations, not constructors.
type Calls = Rc<RefCell<Vec<Value>>>;

/// Records the first reached operation before returning a distinct family failure.
fn reject<T: 'static>(calls: &Calls, family: &str, path: &str) -> OperationFuture<'static, T> {
    calls.borrow_mut().push(json!([family, path]));
    let error = maestro_extensions_wasm::DiagnosticErrorInfo::error(format!("routed {family}"));
    Box::pin(async move { Err(error) })
}

/// Reuses forbidden-family records, replacing only the reached operation with operand capture.
fn operations(calls: &Calls, bytes: Vec<u8>) -> ToolsOptions {
    let effects = super::builtin_selection::Effects::default();
    let mut options = super::builtin_selection::options(&effects);
    let (access, read, mime) = (calls.clone(), calls.clone(), calls.clone());
    options.read.operations = Some(ReadOperations {
        access: Rc::new(move |path| {
            access.borrow_mut().push(json!(["read.access", path]));
            Box::pin(async { Ok(()) })
        }),
        read_file: Rc::new(move |path| {
            read.borrow_mut().push(json!(["read.read_file", path]));
            let bytes = bytes.clone();
            Box::pin(async move { Ok(bytes) })
        }),
        detect_image_mime_type: Some(Rc::new(move |path| {
            mime.borrow_mut().push(json!(["read.mime", path]));
            Box::pin(async { Ok(Some("image/png".into())) })
        })),
    });
    let trace = calls.clone();
    options.bash.operations = Some(BashOperations {
        exec: Rc::new(move |command, cwd, options| {
            trace.borrow_mut().push(json!(["bash.exec", command, cwd]));
            Box::pin(async move {
                (options.on_data)(b"routed shell")?;
                Ok(BashExecResult {
                    exit_code: Some(0.0),
                })
            })
        }),
    });
    options.bash.spawn_hook = None;
    macro_rules! first {
        ($field:ident, $operation:ident, $name:literal) => {{
            let calls = calls.clone();
            if let Some(operations) = &mut options.$field.operations {
                operations.$operation = Rc::new(move |path| reject(&calls, $name, &path));
            }
        }};
    }
    first!(edit, access, "edit.access");
    first!(write, mkdir, "write.mkdir");
    first!(grep, is_directory, "grep.is_directory");
    first!(find, exists, "find.exists");
    first!(ls, exists, "ls.exists");
    options
}

/// Each member and each read/shell control is admitted independently.
fn selected(request: &Value, calls: &Calls) -> ExtensionResult<ToolsOptions> {
    let bytes = serde_json::from_value(request["bytes"].clone()).map_err(|e| e.to_string())?;
    let supplied = operations(calls, bytes);
    let mut options = ToolsOptions::default();
    let members = request["members"]
        .as_array()
        .ok_or("missing option members")?;
    macro_rules! member {
        ($field:ident) => {
            if members.iter().any(|name| name == stringify!($field)) {
                options.$field = supplied.$field;
            }
        };
    }
    member!(read);
    member!(bash);
    member!(edit);
    member!(write);
    member!(grep);
    member!(find);
    member!(ls);
    options.read.auto_resize_images = request["resize"].as_bool();
    options.bash.command_prefix = request["prefix"].as_str().map(str::to_owned);
    options.bash.shell_path = request["shell"].as_str().map(str::to_owned);
    if request["hook"] == true {
        let calls = calls.clone();
        options.bash.spawn_hook = Some(Rc::new(move |spawn, _| {
            calls
                .borrow_mut()
                .push(json!(["bash.hook", spawn.command, spawn.cwd]));
            Ok(spawn)
        }));
    }
    Ok(options)
}

/// Keeps context-free executables distinct from retained context-bearing definitions.
enum Invocation {
    /// Four-argument executable returned by a native factory.
    Tool(Tool),
    /// Complete native definition returned by a native factory.
    Definition(ToolDefinition),
}
impl Invocation {
    /// Supplies only the context the returned projection actually requires.
    async fn run(self, context: &ExtensionCommandContext, args: Value) -> Value {
        let (name, result) = match self {
            Self::Tool(tool) => (
                tool.name,
                (tool.execute)("matrix".into(), args, None, None).await,
            ),
            Self::Definition(tool) => (
                tool.metadata.name,
                (tool.execute)("matrix".into(), args, None, None, (**context).clone()).await,
            ),
        };
        match result {
            Ok(result) => json!({"name":name,"result":result}),
            Err(error) => json!({"name":name,"error":error}),
        }
    }
}

/// Executes every selected member, including default members beside supplied siblings.
pub(super) async fn execute(
    api: &ExtensionAPI,
    context: &ExtensionCommandContext,
    request: &str,
) -> ExtensionResult<String> {
    let request: Value = serde_json::from_str(request).map_err(|e| e.to_string())?;
    let calls = Calls::default();
    let tools = factories(api, &request, &calls)?;
    let mut results = Vec::new();
    for tool in tools {
        let name = match &tool {
            Invocation::Tool(tool) => &tool.name,
            Invocation::Definition(tool) => &tool.metadata.name,
        };
        let args = request["args"][name].clone();
        results.push(tool.run(context, args).await);
    }
    Ok(json!({"calls":*calls.borrow(),"results":results}).to_string())
}

/// Uses each public individual, selector and aggregate factory directly.
fn factories(
    api: &ExtensionAPI,
    request: &Value,
    calls: &Calls,
) -> ExtensionResult<Vec<Invocation>> {
    let cwd = request["cwd"].as_str().ok_or("missing option cwd")?;
    let form = request["form"].as_u64().ok_or("missing factory form")?;
    let options = selected(request, calls)?;
    macro_rules! list {
        ($method:ident, $variant:ident) => {
            api.$method(cwd.into(), options)?
                .into_iter()
                .map(Invocation::$variant)
                .collect()
        };
    }
    let tools = match form {
        0 => individual(api, cwd, options)?,
        1 => definitions(api, cwd, options)?,
        2 | 3 => selectors(api, request, calls)?,
        4 => list!(create_coding_tools, Tool),
        5 => list!(create_coding_tool_definitions, Definition),
        6 => list!(create_read_only_tools, Tool),
        7 => list!(create_read_only_tool_definitions, Definition),
        8 => api
            .create_all_tools(cwd.into(), options)?
            .into_values()
            .map(Invocation::Tool)
            .collect(),
        9 => api
            .create_all_tool_definitions(cwd.into(), options)?
            .into_values()
            .map(Invocation::Definition)
            .collect(),
        _ => return Err("unknown option factory form".into()),
    };
    Ok(tools)
}

/// Constructs each named selector with an independently owned option record.
fn selectors(
    api: &ExtensionAPI,
    request: &Value,
    calls: &Calls,
) -> ExtensionResult<Vec<Invocation>> {
    let cwd = request["cwd"].as_str().ok_or("missing selector cwd")?;
    let mut tools = Vec::new();
    for name in ["read", "bash", "edit", "write", "grep", "find", "ls"] {
        let options = selected(request, calls)?;
        tools.push(if request["form"] == 2 {
            Invocation::Tool(api.create_tool(name, cwd.into(), options)?)
        } else {
            Invocation::Definition(api.create_tool_definition(name, cwd.into(), options)?)
        });
    }
    Ok(tools)
}

/// Moves each supplied member to its individual executable owner exactly once.
fn individual(
    api: &ExtensionAPI,
    cwd: &str,
    options: ToolsOptions,
) -> ExtensionResult<Vec<Invocation>> {
    Ok(vec![
        Invocation::Tool(api.create_read_tool(cwd.into(), options.read)?),
        Invocation::Tool(api.create_bash_tool(cwd.into(), options.bash)?),
        Invocation::Tool(api.create_edit_tool(cwd.into(), options.edit)?),
        Invocation::Tool(api.create_write_tool(cwd.into(), options.write)?),
        Invocation::Tool(api.create_grep_tool(cwd.into(), options.grep)?),
        Invocation::Tool(api.create_find_tool(cwd.into(), options.find)?),
        Invocation::Tool(api.create_ls_tool(cwd.into(), options.ls)?),
    ])
}

/// Moves each supplied member to its individual definition owner exactly once.
fn definitions(
    api: &ExtensionAPI,
    cwd: &str,
    options: ToolsOptions,
) -> ExtensionResult<Vec<Invocation>> {
    Ok(vec![
        Invocation::Definition(api.create_read_tool_definition(cwd.into(), options.read)?),
        Invocation::Definition(api.create_bash_tool_definition(cwd.into(), options.bash)?),
        Invocation::Definition(api.create_edit_tool_definition(cwd.into(), options.edit)?),
        Invocation::Definition(api.create_write_tool_definition(cwd.into(), options.write)?),
        Invocation::Definition(api.create_grep_tool_definition(cwd.into(), options.grep)?),
        Invocation::Definition(api.create_find_tool_definition(cwd.into(), options.find)?),
        Invocation::Definition(api.create_ls_tool_definition(cwd.into(), options.ls)?),
    ])
}
