//! Complete individual factory metadata compared with the native factory owners.
use maestro_extensions::{
    EditToolOptions, ExtensionRunner, FindToolOptions, GrepToolOptions, Local, LsToolOptions,
    ReadToolOptions, WriteToolOptions,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{cell::Cell, rc::Rc, sync::Arc};

/// All seven wrappers retain executable fields only and execute without a context read.
#[test]
fn builtin_wrapped_tools_omit_definition_only_metadata() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let cwd = scratch.path().to_str().unwrap();
        let manager = Arc::new(
            maestro_extensions::ToolsManager::new(cwd.into(), None).with_offline(Some(true)),
        );
        let (runner, _) = super::builtin_author::loaded_with_tool_services(
            maestro_extensions::ToolConstruction {
                tools_manager: manager.clone(),
                bin_dir: cwd.into(),
            },
        )
        .await;
        let args = arguments();
        reset_files(scratch.path());
        let expected = native_observations(cwd, manager, &args).await;
        reset_files(scratch.path());
        let model_reads = forbid_model_reads(&runner);
        let command = runner.get_command("bridge-observe").unwrap().command;
        let request = json!({"cwd":cwd,"args":args});
        let result = (command.handler)(
            format!("builtin-wrapped {request}"),
            runner.create_command_context(),
        )
        .await;
        let observed: Value = serde_json::from_str(&result.unwrap_err().to_string()).unwrap();
        assert!(
            expected.iter().all(|tool| tool.get("result").is_some()),
            "{expected:#?}"
        );
        assert_eq!(observed, json!(expected));
        assert_eq!(model_reads.get(), 0);
        assert_eq!(
            std::fs::read(scratch.path().join("written.txt")).unwrap(),
            "é😀\0".as_bytes()
        );
        assert_eq!(
            std::fs::read_to_string(scratch.path().join("edit.txt")).unwrap(),
            "after 雪\n"
        );
        assert_registered_definitions(&runner);
    });
}

/// Each independently returned definition survives real registration with an open custom name.
fn assert_registered_definitions(runner: &ExtensionRunner<Local>) {
    let names: Vec<_> = runner
        .get_all_registered_tools()
        .into_iter()
        .map(|tool| tool.definition.name())
        .filter(|name| name.starts_with("factory-"))
        .collect();
    assert_eq!(
        names,
        [
            "factory-read",
            "factory-bash",
            "factory-edit",
            "factory-write",
            "factory-grep",
            "factory-find",
            "factory-ls"
        ]
    );
}

/// Literal valid operands reach execution rather than stopping at schema validation.
fn arguments() -> Vec<Value> {
    vec![
        json!({"path":"read.txt"}),
        json!({"command":"printf 'wrapped é😀'"}),
        json!({"path":"edit.txt","edits":[{"oldText":"before","newText":"after"}]}),
        json!({"path":"written.txt","content":"é😀\0"}),
        json!({"pattern":"needle","path":"search","context":1}),
        json!({"pattern":"*.txt","path":"search"}),
        json!({"path":"search"}),
    ]
}

/// Restores the same native input bytes before each independent execution sequence.
fn reset_files(root: &std::path::Path) {
    std::fs::create_dir_all(root.join("search")).unwrap();
    std::fs::write(root.join("search/match.txt"), "first\nneedle 雪\nlast\n").unwrap();
    std::fs::write(root.join("read.txt"), "read é😀\0\n").unwrap();
    std::fs::write(root.join("edit.txt"), "before 雪\n").unwrap();
    let written = root.join("written.txt");
    if written.exists() {
        std::fs::remove_file(written).unwrap();
    }
}

/// A failing reader makes any unexpected executable context dependency visible.
fn forbid_model_reads(runner: &ExtensionRunner<Local>) -> Rc<Cell<u32>> {
    let reads = Rc::new(Cell::new(0));
    let observed = reads.clone();
    runner
        .bind_core(
            <Local as maestro_extensions::ExtensionExecution>::unbound_actions(),
            maestro_extensions::ExtensionContextActions {
                get_model: Rc::new(move || {
                    observed.set(observed.get() + 1);
                    Err("wrapped execution has no context".into())
                }),
                ..Default::default()
            },
            maestro_extensions::types::ProviderActions::default(),
        )
        .unwrap();
    reads
}

/// Uses each individual native definition as the metadata and execution comparator.
async fn native_observations(
    cwd: &str,
    manager: Arc<maestro_extensions::ToolsManager>,
    args: &[Value],
) -> Vec<Value> {
    use maestro_extensions::local::*;
    vec![
        native_observation(
            create_read_tool_definition::<()>(cwd.into(), ReadToolOptions::default()),
            &args[0],
        )
        .await,
        native_observation(
            create_bash_tool_definition::<()>(cwd.into(), cwd.into(), BashToolOptions::default()),
            &args[1],
        )
        .await,
        native_observation(
            create_edit_tool_definition::<()>(cwd.into(), EditToolOptions::default()),
            &args[2],
        )
        .await,
        native_observation(
            create_write_tool_definition::<()>(cwd.into(), WriteToolOptions::default()),
            &args[3],
        )
        .await,
        native_observation(
            create_grep_tool_definition::<()>(
                cwd.into(),
                GrepToolOptions::default(),
                manager.clone(),
            ),
            &args[4],
        )
        .await,
        native_observation(
            create_find_tool_definition::<()>(cwd.into(), FindToolOptions::default(), manager),
            &args[5],
        )
        .await,
        native_observation(
            create_ls_tool_definition::<()>(cwd.into(), LsToolOptions::default()),
            &args[6],
        )
        .await,
    ]
}

/// Reads the native definition before wrapping, without using the adapter's metadata projection.
async fn native_observation<P, D, S>(
    definition: maestro_extensions::local::ToolDefinition<(), P, D, S>,
    args: &Value,
) -> Value
where
    P: DeserializeOwned + 'static,
    D: Serialize + 'static,
    S: Default + 'static,
{
    let mut metadata = serde_json::to_value(&definition.definition).unwrap();
    metadata["label"] = json!(definition.label);
    if let Some(mode) = definition.execution_mode {
        metadata["executionMode"] = json!(mode);
    }
    let mut complete = metadata.clone();
    if let Some(snippet) = &definition.prompt_snippet {
        complete["promptSnippet"] = json!(snippet);
    }
    if let Some(guidelines) = &definition.prompt_guidelines {
        complete["promptGuidelines"] = json!(guidelines);
    }
    if let Some(shell) = definition.render_shell {
        complete["renderShell"] = json!(match shell {
            maestro_extensions::ToolRenderShell::Default => "default",
            maestro_extensions::ToolRenderShell::SelfRendered => "self",
        });
    }
    let mut observation = json!({"metadata":metadata,"prepares":definition.prepare_arguments.is_some(),
        "definition":complete,"definitionPrepares":definition.prepare_arguments.is_some(),
        "renderers":[definition.render_call.is_some(),definition.render_result.is_some()]});
    let tool = maestro_extensions::local::wrap_tool_definition(definition, None);
    match (tool.execute)("wrapped-factory".into(), args.clone(), None, None).await {
        Ok(result) => observation["result"] = serde_json::to_value(result).unwrap(),
        Err(error) => observation["error"] = json!(error.to_string()),
    }
    observation
}

/// Both selectors reject exact non-built-in spellings without probing the supplied directory.
#[test]
fn builtin_unknown_names_preserve_the_original_spelling() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let runner = super::builtin_author::loaded().await;
        let names = [
            "",
            "Read",
            "READ",
            " read",
            "read ",
            "\u{feff}read",
            "read\u{85}",
            "custom 雪",
            "__proto__",
            "constructor",
            "prototype",
            "read\n",
            "read\0",
            "réad",
            "雪",
        ];
        let expected: Vec<_> = names
            .iter()
            .map(|name| {
                let message = format!("Unknown tool name: {name}");
                json!([message, message])
            })
            .collect();
        let command = runner.get_command("bridge-observe").unwrap().command;
        let result = (command.handler)(
            format!("builtin-unknown {}", json!(names)),
            runner.create_command_context(),
        )
        .await;
        let observed: Value = serde_json::from_str(&result.unwrap_err().to_string()).unwrap();
        assert_eq!(observed, json!({"errors": expected, "effects": []}));
        assert!(
            runner
                .get_all_registered_tools()
                .iter()
                .any(|tool| tool.definition.name() == "custom 雪")
        );
    });
}

/// Ordered observations reject swapped or sorted presets and preserve all seven selectors.
#[test]
fn builtin_factories_preserve_membership_and_order() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let runner = super::builtin_author::loaded().await;
        let command = runner.get_command("bridge-observe").unwrap().command;
        let names = ["read", "bash", "edit", "write", "grep", "find", "ls"];
        let keys = ["Read", "Bash", "Edit", "Write", "Grep", "Find", "Ls"];
        let pairs: Vec<_> = names.iter().map(|name| json!([name,name])).collect();
        let keyed: Vec<_> = keys.iter().zip(names).map(|(key,name)| json!([key,name])).collect();
        let expected = json!({"individual":pairs,"selected":pairs,"coding":["read","bash","edit","write"],"codingDefinitions":["read","bash","edit","write"],"readOnly":["read","grep","find","ls"],"readOnlyDefinitions":["read","grep","find","ls"],"all":keyed,"allDefinitions":keyed,"names":keys});
        for cwd in ["", "relative/../雪", "./a/..", "/absent-factory-root", "C:\\Tools\\雪"] {
            let result = (command.handler)(format!("builtin-membership {cwd}"), runner.create_command_context()).await;
            let observed: Value = serde_json::from_str(&result.unwrap_err().to_string()).unwrap();
            assert_eq!(observed, expected, "cwd={cwd:?}");
        }
    });
}
