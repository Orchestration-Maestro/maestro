//! Authored agents through their real component and native filesystem owners.
#![cfg(test)]

use crate::support::Root;
mod cancellation;
mod errors;
mod render;
mod trust;
use maestro_app::AgentSession;
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::{path::Path, rc::Rc, sync::OnceLock};

#[path = "../../maestro-extensions-wasm/src/tests/build/examples.rs"]
mod component_build;

/// Loads the package using the existing application session and native-I/O fixture.
async fn loaded(root: &Root, local: Rc<tokio::task::LocalSet>) -> Rc<AgentSession> {
    static COMPONENT: OnceLock<std::path::PathBuf> = OnceLock::new();
    let path = COMPONENT.get_or_init(|| {
        component_build::example_component("maestro-subagents", "component").unwrap()
    });
    crate::support::session::component_session(
        root,
        local,
        path.display().to_string(),
        Some(maestro_app::NoTools::Builtin),
        crate::support::widgets(),
    )
    .await
}

/// Invokes the registered definition through the application's tool wrapper.
async fn execute(session: &Rc<AgentSession>, args: Value) -> Value {
    invoked(session, args, None, None).await.unwrap()
}

/// Retains the public invocation's independent signal and progress capabilities.
async fn invoked(
    session: &Rc<AgentSession>,
    args: Value,
    signal: Option<maestro_app::Cancellation>,
    update: Option<maestro_app::AgentToolUpdateCallback<Value, maestro_app::Local>>,
) -> Result<Value, String> {
    let runner = session.extension_runner();
    let registered = runner
        .get_all_registered_tools()
        .into_iter()
        .find(|tool| tool.definition.name() == "subagent")
        .unwrap();
    let tool = maestro_app::wrap_registered_tool(&registered, runner);
    (tool.execute)("delegated".into(), args, signal, update)
        .await
        .map(|result| serde_json::to_value(result).unwrap())
        .map_err(|error| error.to_string())
}

/// Runs the real application effects on their retained caller-owned executor.
fn run(
    action: impl FnOnce(Rc<tokio::task::LocalSet>) -> std::pin::Pin<Box<dyn Future<Output = ()>>>,
) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, action(Rc::clone(&local)));
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
    run(|local| {
        Box::pin(async move {
            let root = Root::new();
            let user = root.path("agent/agents");
            agent(&user, "role.md", "role");
            agent(&root.path(".maestro/agents"), "outer.md", "outer");
            agent(&user, "ignored.txt", "ignored");
            std::fs::write(user.join("invalid.md"), "---\nname: invalid\n---\n").unwrap();
            let session = loaded(&root, local).await;
            let default = execute(&session, json!({})).await;
            assert_eq!(
                default["content"][0]["text"],
                "Invalid parameters. Provide exactly one mode.\nAvailable agents: role (user)"
            );
            assert_eq!(
                default["details"]["projectAgentsDir"],
                root.path(".maestro/agents").to_str().unwrap()
            );
            #[cfg(unix)]
            {
                std::fs::rename(user.join("role.md"), root.path("linked-role.md")).unwrap();
                std::os::unix::fs::symlink(root.path("linked-role.md"), user.join("role.md"))
                    .unwrap();
                assert_eq!(
                    execute(&session, json!({})).await["content"],
                    default["content"]
                );
            }
            agent(&root.path("project/.maestro/agents"), "role.md", "role");
            #[cfg(unix)]
            {
                std::fs::rename(
                    root.path("project/.maestro/agents"),
                    root.path("linked-agents"),
                )
                .unwrap();
                std::os::unix::fs::symlink(
                    root.path("linked-agents"),
                    root.path("project/.maestro/agents"),
                )
                .unwrap();
            }
            let both = execute(&session, json!({"agentScope":"both"})).await;
            assert_eq!(
                both["content"][0]["text"],
                "Invalid parameters. Provide exactly one mode.\nAvailable agents: role (project)"
            );
            let project_only = execute(&session, json!({"agentScope":"project"})).await;
            assert_eq!(project_only["content"], both["content"]);
            std::fs::remove_file(user.join("role.md")).unwrap();
            let refreshed = execute(&session, json!({})).await;
            assert_eq!(
                refreshed["content"][0]["text"],
                "Invalid parameters. Provide exactly one mode.\nAvailable agents: none"
            );
        })
    });
}

/// Starts a selected test in its own environment, keeping its child script private.
#[cfg(unix)]
fn child_fixture(test: &str, component: Option<&Path>) -> Option<Scratch> {
    use std::os::unix::fs::PermissionsExt;
    if maestro_test_process::selection().is_child() {
        return None;
    }
    let root = Scratch::new().unwrap();
    std::fs::create_dir(root.path().join("tmp")).unwrap();
    let program = root.path().join("maestro-child");
    std::fs::write(&program, include_str!("fixtures/subagent-child.py")).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    let status = maestro_test_process::TestProcess::current(test)
        .unwrap()
        .case("*")
        .env("MAESTRO_BIN_PATH", &program)
        .env("TMPDIR", root.path().join("tmp"))
        .envs(component.map(|path| ("MAESTRO_NATIVE_IO_COMPONENT", path)))
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
async fn modes(runner: &Rc<AgentSession>, root: &Path) {
    let single = execute(runner, json!({"agent":"role","task":"inspect"})).await;
    let text = "Task: inspect;role=Private role prompt;cwd=project";
    assert_eq!(single["content"][0]["text"], text);
    assert_eq!(
        single["details"]["results"][0]["usage"],
        json!({"input":12.0,"output":8.0,"cacheRead":4.0,"cacheWrite":6.0,"cost":0.003,"contextTokens":19.0,"turns":2})
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
    if child_fixture(
        "subagent_example::subagent_invocation_modes_preserve_child_output_and_usage",
        None,
    )
    .is_some()
    {
        return;
    }
    run(|local| {
        Box::pin(async move {
            let root = Root::new();
            std::fs::create_dir(root.path("other")).unwrap();
            std::fs::create_dir_all(root.path("agent/agents")).unwrap();
            std::fs::write(root.path("agent/agents/role.md"), "---\nname: role\ndescription: Controlled child\nmodel: controlled-model\ntools: read, ls\n---\nPrivate role prompt\n").unwrap();
            let session = loaded(&root, local).await;
            modes(&session, root.scratch.path()).await;
        })
    });
}

/// Writes the complete controlled role for the selected discovery source.
fn controlled_role(root: &Root, directory: &str) {
    std::fs::create_dir_all(root.path(directory)).unwrap();
    std::fs::write(root.path(directory).join("role.md"), "---\nname: role\ndescription: Controlled child\nmodel: controlled-model\ntools: read, ls\n---\nPrivate role prompt\n").unwrap();
}
