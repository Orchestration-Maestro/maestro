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
    if !super::builtin_author::with_search_tools(
        "builtin_wrapped_tools_omit_definition_only_metadata",
    ) {
        return;
    }
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

/// Construction closes without effects; releasing another set leaves every owner executable.
#[test]
fn builtin_factory_construction_is_lazy_and_fresh() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let runner = super::builtin_author::loaded().await;
        let reads = Rc::new(Cell::new(0));
        let observed = reads.clone();
        runner
            .bind_core(
                <Local as maestro_extensions::ExtensionExecution>::unbound_actions(),
                maestro_extensions::ExtensionContextActions {
                    get_model: Rc::new(move || {
                        observed.set(observed.get() + 1);
                        Ok(None)
                    }),
                    ..Default::default()
                },
                maestro_extensions::types::ProviderActions::default(),
            )
            .unwrap();
        let command = runner.get_command("bridge-observe").unwrap().command;
        let result = (command.handler)(
            "builtin-fresh /absent-lazy-root/雪".into(),
            runner.create_command_context(),
        )
        .await;
        let observed: Value = serde_json::from_str(&result.unwrap_err().to_string()).unwrap();
        assert_eq!(
            observed,
            json!({"construction":[], "first":[
            "read.access","read.access","bash.hook","bash.hook",
            "edit.access","edit.access","write.mkdir","write.mkdir",
            "grep.is_directory","grep.is_directory","find.exists","find.exists",
            "ls.exists","ls.exists"],"second":[],"failed":vec![true;14]})
        );
        assert_eq!(
            reads.get(),
            1,
            "only the executed read definition consults a model"
        );
    });
}

/// Complete native metadata and callbacks survive the final registration consumer.
#[test]
fn builtin_definitions_preserve_metadata_and_render_handles() {
    if !super::builtin_author::with_search_tools(
        "builtin_definitions_preserve_metadata_and_render_handles",
    ) {
        return;
    }
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
        let runner = Rc::new(runner);
        let args = arguments();
        reset_files(scratch.path());
        let expected = native_observations(cwd, manager, &args).await;
        reset_files(scratch.path());
        let command = runner.get_command("bridge-observe").unwrap().command;
        let request = json!({"cwd":cwd,"args":args});
        let result = (command.handler)(
            format!("builtin-wrapped {request}"),
            runner.create_command_context(),
        )
        .await;
        let observed: Value = serde_json::from_str(&result.unwrap_err().to_string()).unwrap();
        assert_eq!(observed, json!(expected));
        reset_files(scratch.path());
        let tools: Vec<_> = runner
            .get_all_registered_tools()
            .into_iter()
            .filter(|tool| tool.definition.name().starts_with("factory-"))
            .collect();
        assert_eq!(tools.len(), 7);
        for ((registered, expected), args) in tools.iter().zip(&expected).zip(args) {
            registered_observation(&runner, registered, expected, args).await;
        }
        assert_eq!(
            std::fs::read(scratch.path().join("written.txt")).unwrap(),
            "é😀\0".as_bytes()
        );
        assert_eq!(
            std::fs::read_to_string(scratch.path().join("edit.txt")).unwrap(),
            "after 雪\n"
        );
    });
}

/// Compares every optional field before preparing and executing the retained registration.
async fn registered_observation(
    runner: &Rc<ExtensionRunner<Local>>,
    registered: &maestro_extensions::types::RegisteredTool<Local>,
    expected: &Value,
    mut args: Value,
) {
    let mut metadata = registered_metadata(registered.definition.metadata());
    assert_eq!(
        metadata["name"],
        format!(
            "factory-{}",
            expected["definition"]["name"].as_str().unwrap()
        )
    );
    metadata["name"] = expected["definition"]["name"].clone();
    assert_eq!(metadata, expected["definition"]);
    let binding = registered.definition.renderer_binding().unwrap();
    assert_eq!(
        json!([
            binding.render_call.is_some(),
            binding.render_result.is_some()
        ]),
        expected["renderers"]
    );
    let tool = maestro_extensions::wrap_registered_tool(registered, runner.clone());
    assert_eq!(
        json!(tool.prepare_arguments.is_some()),
        expected["definitionPrepares"]
    );
    if let Some(prepare) = &tool.prepare_arguments {
        args = prepare(&mut args).unwrap();
    }
    let result = (tool.execute)("registered-factory".into(), args, None, None)
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(result).unwrap(), expected["result"]);
}

/// Projects the real registration's metadata without filling absent fields with null.
fn registered_metadata(metadata: maestro_extensions::types::ToolDefinitionMetadata) -> Value {
    let mut value = serde_json::to_value(metadata.definition).unwrap();
    value["label"] = json!(metadata.label);
    if let Some(snippet) = metadata.prompt_snippet {
        value["promptSnippet"] = json!(snippet);
    }
    if let Some(guidelines) = metadata.prompt_guidelines {
        value["promptGuidelines"] = json!(guidelines);
    }
    if let Some(shell) = metadata.render_shell {
        value["renderShell"] = json!(match shell {
            maestro_extensions::ToolRenderShell::Default => "default",
            maestro_extensions::ToolRenderShell::SelfRendered => "self",
        });
    }
    if let Some(mode) = metadata.execution_mode {
        value["executionMode"] = json!(mode);
    }
    value
}

/// Every factory executes defaults, independent members and unequal read/shell controls.
#[test]
fn builtin_options_reach_only_the_selected_operations() {
    if !super::builtin_author::with_search_tools(
        "builtin_options_reach_only_the_selected_operations",
    ) {
        return;
    }
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
        let mut args = arguments();
        args[1] = json!({"command":"printf default"});
        reset_files(scratch.path());
        std::fs::write(
            scratch.path().join("read.txt"),
            include_bytes!("read_resize.png"),
        )
        .unwrap();
        let expected = native_observations(cwd, manager, &args).await;
        assert!(
            expected.iter().all(|tool| tool.get("result").is_some()),
            "{expected:#?}"
        );
        for case in option_cases() {
            for form in 0..10 {
                reset_matrix_files(scratch.path());
                let mut request = case.clone();
                request["cwd"] = json!(cwd);
                request["form"] = json!(form);
                request["bytes"] = json!(include_bytes!("read_resize.png").as_slice());
                request["args"] = ["read", "bash", "edit", "write", "grep", "find", "ls"]
                    .into_iter()
                    .zip(args.iter().cloned())
                    .collect();
                let command = runner.get_command("bridge-observe").unwrap().command;
                let result = (command.handler)(
                    format!("builtin-options {request}"),
                    runner.create_command_context(),
                )
                .await;
                let observed: Value =
                    serde_json::from_str(&result.unwrap_err().to_string()).unwrap();
                assert_option_execution(&observed, &request, &expected).await;
            }
        }
    });
}

/// Typed default construction has no separate missing-member wire representation.
fn option_cases() -> Vec<Value> {
    let names = ["read", "bash", "edit", "write", "grep", "find", "ls"];
    let mut cases = vec![json!({"members":[]})];
    cases.extend(names.map(|name| json!({"members":[name]})));
    cases.extend([
        json!({"members":names}),
        json!({"members":[],"resize":true}),
        json!({"members":[],"resize":false}),
        json!({"members":[],"prefix":"printf prefix;"}),
        json!({"members":[],"shell":"/missing-matrix-shell"}),
        json!({"members":[],"hook":true}),
        json!({"members":names,"resize":false,"prefix":"printf prefix;","shell":"/missing-matrix-shell","hook":true}),
        json!({"members":names,"resize":true,"prefix":"","hook":true}),
    ]);
    cases
}

/// Independent literal callback operands detect option leakage and wrong selected cwd.
async fn assert_option_execution(observed: &Value, request: &Value, defaults: &[Value]) {
    let mut calls = Vec::new();
    let members = request["members"].as_array().unwrap();
    let cwd = request["cwd"].as_str().unwrap();
    let names = ["read", "bash", "edit", "write", "grep", "find", "ls"];
    let selected: &[usize] = match request["form"].as_u64().unwrap() {
        4 | 5 => &[0, 1, 2, 3],
        6 | 7 => &[0, 4, 5, 6],
        _ => &[0, 1, 2, 3, 4, 5, 6],
    };
    let results = observed["results"].as_array().unwrap();
    assert_eq!(results.len(), selected.len());
    for (result, &index) in results.iter().zip(selected) {
        let name = names[index];
        assert_eq!(result["name"], name);
        let supplied = members.iter().any(|member| member == name);
        if index == 1 {
            assert_matrix_bash(result, request, &mut calls).await;
        } else if index == 0 {
            if supplied {
                calls.extend(
                    ["read.access", "read.mime", "read.read_file"]
                        .map(|stage| json!([stage, joined(cwd, "read.txt")])),
                );
            }
            let expected = super::builtin_operations::native_read_case(
                &json!({"bytes":request["bytes"],"mime":"image/png","resize":request["resize"]}),
                None,
            )
            .await;
            assert_eq!(result["result"], expected, "read form {}", request["form"]);
        } else if supplied {
            calls.push(matrix_first_call(index, cwd));
            assert!(result["error"].is_string(), "{result}");
        } else {
            assert_eq!(
                result["result"], defaults[index]["result"],
                "{name}, {request}"
            );
        }
    }
    assert_eq!(observed["calls"], json!(calls), "case {request}");
}

/// Restores mutations while retaining the same resize-sensitive native input.
fn reset_matrix_files(root: &std::path::Path) {
    reset_files(root);
    std::fs::write(root.join("read.txt"), include_bytes!("read_resize.png")).unwrap();
}

/// `name` below `cwd`, joined with the host separator as the native tools join it.
fn joined(cwd: &str, name: &str) -> String {
    std::path::Path::new(cwd).join(name).display().to_string()
}

/// Literal first-phase operands for families whose deeper behavior has separate witnesses.
fn matrix_first_call(index: usize, cwd: &str) -> Value {
    let (stage, path) = match index {
        2 => ("edit.access", joined(cwd, "edit.txt")),
        3 => ("write.mkdir", cwd.to_owned()),
        4 => ("grep.is_directory", joined(cwd, "search")),
        5 => ("find.exists", joined(cwd, "search")),
        6 => ("ls.exists", joined(cwd, "search")),
        _ => panic!("not a failing matrix family"),
    };
    json!([stage, path])
}

/// Prefix/hook operands and final shell output are consumed independently of image policy.
async fn assert_matrix_bash(result: &Value, request: &Value, calls: &mut Vec<Value>) {
    let cwd = request["cwd"].as_str().unwrap();
    let command = if request["prefix"].as_str().is_some_and(|p| !p.is_empty()) {
        "printf prefix;\nprintf default"
    } else {
        "printf default"
    };
    if request["hook"] == true {
        calls.push(json!(["bash.hook", command, cwd]));
    }
    if request["members"]
        .as_array()
        .unwrap()
        .iter()
        .any(|name| name == "bash")
    {
        calls.push(json!(["bash.exec", command, cwd]));
        assert_eq!(
            result["result"],
            json!({"content":[{"type":"text","text":"routed shell"}]})
        );
        return;
    }
    let definition = maestro_extensions::local::create_bash_tool_definition::<()>(
        cwd.into(),
        cwd.into(),
        maestro_extensions::local::BashToolOptions {
            command_prefix: request["prefix"].as_str().map(str::to_owned),
            shell_path: request["shell"].as_str().map(str::to_owned),
            ..Default::default()
        },
    );
    let tool = maestro_extensions::local::wrap_tool_definition(definition, None);
    match (tool.execute)(
        "matrix-native".into(),
        json!({"command":"printf default"}),
        None,
        None,
    )
    .await
    {
        Ok(expected) => assert_eq!(result["result"], serde_json::to_value(expected).unwrap()),
        Err(expected) => assert_eq!(result["error"], expected.to_string()),
    }
}
