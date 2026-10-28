//! Authored agents through their real component and native filesystem owners.
#![cfg(test)]

use maestro_extensions::{ExtensionRunner, Local};
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::{
    path::Path,
    rc::Rc,
    sync::{OnceLock, RwLock},
};

use super::component_build;

/// A package component loaded with a caller-selected agent directory.
async fn loaded(cwd: &Path, agent_dir: &Path) -> Rc<ExtensionRunner<Local>> {
    static COMPONENT: OnceLock<std::path::PathBuf> = OnceLock::new();
    let path = COMPONENT
        .get_or_init(|| {
            component_build::example_component("maestro-subagents", "component").unwrap()
        })
        .display()
        .to_string();
    let runtime = maestro_extensions::create_extension_runtime::<Local>();
    let bus = super::components::TracedBus::new();
    let owner = maestro_extensions::load_extension_from_factory(
        maestro_extensions_wasmtime::artifact_factory(
            path.clone(),
            super::support::widgets(),
            maestro_extensions_wasmtime::Installation {
                version: "subagent-test".into(),
                agent_dir: agent_dir.to_str().unwrap().into(),
            },
        ),
        cwd.to_str().unwrap().into(),
        bus.controller.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    let runner = Rc::new(ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        super::session::runner_session(cwd.to_str().unwrap()).0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    ));
    super::support::bind_prompt(&runner, "authored agent test");
    runner
}

/// Invokes the registered definition through the application's tool wrapper.
async fn execute(runner: &Rc<ExtensionRunner<Local>>, args: Value) -> Value {
    let registered = runner
        .get_all_registered_tools()
        .into_iter()
        .find(|tool| tool.definition.name() == "subagent")
        .unwrap();
    let tool = maestro_extensions::wrap_registered_tool(&registered, runner.clone());
    let result = (tool.execute)("delegated".into(), args, None, None)
        .await
        .unwrap();
    serde_json::to_value(result).unwrap()
}

/// Writes an authored definition without relying on the user's home configuration.
fn agent(directory: &Path, file: &str, name: &str) {
    std::fs::create_dir_all(directory).unwrap();
    std::fs::write(
        directory.join(file),
        format!("---\nname: {name}\ndescription: Controlled role\n---\nRole prompt\n"),
    )
    .unwrap();
}

#[test]
fn subagent_discovery_refreshes_scopes_and_nearest_project_shadowing() {
    super::example_process::run(async {
        let root = Scratch::new().unwrap();
        let user = root.path().join("user");
        let project = root.path().join("project");
        let cwd = project.join("nested/work");
        std::fs::create_dir_all(&cwd).unwrap();
        agent(&user.join("agents"), "role.md", "role");
        agent(&project.join(".maestro/agents"), "role.md", "role");
        agent(&root.path().join(".maestro/agents"), "outer.md", "outer");
        agent(&user.join("agents"), "ignored.txt", "ignored");
        std::fs::write(user.join("agents/invalid.md"), "---\nname: invalid\n---\n").unwrap();
        let runner = loaded(&cwd, &user).await;
        let default = execute(&runner, json!({})).await;
        assert_eq!(
            default["content"][0]["text"],
            "Invalid parameters. Provide exactly one mode.\nAvailable agents: role (user)"
        );
        let both = execute(&runner, json!({"agentScope":"both"})).await;
        assert_eq!(
            both["content"][0]["text"],
            "Invalid parameters. Provide exactly one mode.\nAvailable agents: role (project)"
        );
        let project_only = execute(&runner, json!({"agentScope":"project"})).await;
        assert_eq!(project_only["content"], both["content"]);
        std::fs::remove_file(user.join("agents/role.md")).unwrap();
        let refreshed = execute(&runner, json!({})).await;
        assert_eq!(
            refreshed["content"][0]["text"],
            "Invalid parameters. Provide exactly one mode.\nAvailable agents: none"
        );
    });
}

/// Starts a selected test in its own environment, keeping its child script private.
#[cfg(unix)]
fn child_fixture(test: &str) -> Option<Scratch> {
    use std::os::unix::fs::PermissionsExt;
    if maestro_test_process::selection().is_child() {
        return None;
    }
    let root = Scratch::new().unwrap();
    for directory in ["work", "other", "tmp", "user/agents"] {
        std::fs::create_dir_all(root.path().join(directory)).unwrap();
    }
    std::fs::write(root.path().join("user/agents/role.md"), "---\nname: role\ndescription: Controlled child\nmodel: controlled-model\ntools: read, ls\n---\nPrivate role prompt\n").unwrap();
    let program = root.path().join("maestro-child");
    std::fs::write(&program, include_str!("fixtures/subagent-child.py")).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    let status = maestro_test_process::TestProcess::current(test)
        .unwrap()
        .case("*")
        .env("MAESTRO_BIN_PATH", &program)
        .env("MAESTRO_SUBAGENT_FIXTURE", root.path())
        .env("TMPDIR", root.path().join("tmp"))
        .nocapture()
        .status()
        .unwrap();
    assert!(status.success(), "{status}");
    assert_eq!(
        std::fs::read_dir(root.path().join("tmp")).unwrap().count(),
        0
    );
    Some(root)
}

/// One set of literal subprocess expectations across all invocation modes.
#[cfg(unix)]
async fn modes(runner: &Rc<ExtensionRunner<Local>>, root: &Path) {
    let single = execute(runner, json!({"agent":"role","task":"inspect"})).await;
    let text = "Task: inspect;role=Private role prompt;cwd=work";
    assert_eq!(single["content"][0]["text"], text);
    assert_eq!(
        single["details"]["results"][0]["usage"],
        json!({"input":12,"output":8,"cacheRead":4,"cacheWrite":6,"cost":0.003,"contextTokens":19,"turns":2})
    );
    assert_eq!(
        single["details"]["results"][0]["stderr"],
        "controlled stderr"
    );
    assert_eq!(
        single["details"]["results"][0]["messages"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let chain = execute(runner, json!({"chain":[{"agent":"role","task":"inspect"},{"agent":"role","task":"continue {previous}","cwd":root.join("other")}]})).await;
    assert_eq!(
        chain["details"]["results"][1]["task"],
        format!("continue {text}")
    );
    assert_eq!(
        chain["content"][0]["text"],
        format!("Task: continue {text};role=Private role prompt;cwd=other")
    );
    let parallel = execute(
        runner,
        json!({"tasks":[{"agent":"role","task":"first"},{"agent":"role","task":"second"}]}),
    )
    .await;
    assert!(
        parallel["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("Parallel: 2/2 succeeded")
    );
    assert_eq!(parallel["details"]["results"][0]["task"], "first");
    assert_eq!(parallel["details"]["results"][1]["task"], "second");
}

#[test]
#[cfg(unix)]
fn subagent_invocation_modes_preserve_child_output_and_usage() {
    if child_fixture("subagent_example::subagent_invocation_modes_preserve_child_output_and_usage")
        .is_some()
    {
        return;
    }
    super::example_process::run(async {
        let root = std::path::PathBuf::from(std::env::var_os("MAESTRO_SUBAGENT_FIXTURE").unwrap());
        let runner = loaded(&root.join("work"), &root.join("user")).await;
        modes(&runner, &root).await;
    });
}
