//! Minimal displays through the shipped component and original native tool owners.
#![cfg(test)]
#![cfg(unix)]
use super::example_process::run;
use maestro_extensions::{ExtensionRunner, Local};
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::{
    os::unix::fs::PermissionsExt as _,
    path::Path,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Loads the one shipped example with native factories and a disposable session cwd.
async fn loaded(cwd: &Path) -> Rc<ExtensionRunner<Local>> {
    let path = super::extension_guide::cargo::component(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../maestro-extensions-wasm"
        )),
        &["--example", "minimal_mode"],
        "minimal_mode",
    )
    .unwrap();
    let path = path.to_str().unwrap().to_owned();
    let runtime = maestro_extensions::create_extension_runtime::<Local>();
    let bin_dir = cwd.join("bin").to_str().unwrap().to_owned();
    runtime.set_tool_construction(maestro_extensions::ToolConstruction {
        tools_manager: Arc::new(
            maestro_extensions::ToolsManager::new(bin_dir.clone(), None).with_offline(Some(true)),
        ),
        bin_dir,
    });
    let bus = maestro_extensions::create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no background work")),
        Rc::new(|error| panic!("{error}")),
    );
    let owner = maestro_extensions::load_extension_from_factory(
        crate::artifact(path.clone()),
        cwd.to_str().unwrap().into(),
        bus.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    Rc::new(ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        super::session::runner_session(cwd.to_str().unwrap()).0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    ))
}

/// Invokes a registered override, not a replacement implementation in the test.
async fn execute(runner: &Rc<ExtensionRunner<Local>>, name: &str, args: Value) -> Value {
    let registered = runner
        .get_all_registered_tools()
        .into_iter()
        .find(|tool| tool.definition.name() == name)
        .unwrap();
    let tool = maestro_extensions::wrap_registered_tool(&registered, runner.clone());
    let result = (tool.execute)("minimal".into(), args, None, None)
        .await
        .unwrap();
    serde_json::to_value(result).unwrap()
}

/// Supplies offline search protocol output beside the disposable input file.
fn search_programs(root: &Path) {
    let bin = root.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let input = root.join("input.txt");
    let event = json!({"type":"match","data":{"path":{"text":input},"line_number":2,"lines":{"text":"two\n"}}});
    for (name, script) in [
        (
            "fd",
            format!("#!/bin/sh\nprintf '%s\\0' '{}'\n", input.display()),
        ),
        ("rg", format!("#!/bin/sh\nprintf '%s\\n' '{event}'\n")),
    ] {
        let path = bin.join(name);
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn minimal_mode_retains_builtin_execution_in_the_session_directory() {
    run(async {
        let root = Scratch::new().unwrap();
        std::fs::write(root.path().join("input.txt"), "one\ntwo\nthree").unwrap();
        search_programs(root.path());
        let runner = loaded(root.path()).await;
        let names: Vec<_> = runner
            .get_all_registered_tools()
            .iter()
            .map(|tool| tool.definition.name())
            .collect();
        assert_eq!(
            names,
            ["read", "bash", "write", "edit", "find", "grep", "ls"]
        );
        for _ in 0..2 {
            let result = execute(
                &runner,
                "read",
                json!({"path":"input.txt","offset":2,"limit":1}),
            )
            .await;
            assert_eq!(
                result["content"][0]["text"],
                "two\n\n[1 more lines in file. Use offset=3 to continue.]"
            );
        }
        execute(
            &runner,
            "write",
            json!({"path":"nested/out.txt","content":"before"}),
        )
        .await;
        execute(
            &runner,
            "edit",
            json!({"path":"nested/out.txt","edits":[{"oldText":"before","newText":"after"}]}),
        )
        .await;
        assert_eq!(
            std::fs::read_to_string(root.path().join("nested/out.txt")).unwrap(),
            "after"
        );
        let result = execute(&runner, "bash", json!({"command":"printf controlled"})).await;
        assert_eq!(result["content"][0]["text"], "controlled");
        let result = execute(&runner, "find", json!({"pattern":"*.txt"})).await;
        assert_eq!(result["content"][0]["text"], "input.txt");
        let result = execute(&runner, "grep", json!({"pattern":"two"})).await;
        assert_eq!(result["content"][0]["text"], "input.txt:2: two");
        let result = execute(&runner, "ls", json!({"path":"nested"})).await;
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("out.txt")
        );
    });
}

#[test]
fn minimal_mode_preserves_cancellation_and_native_read_limits() {
    run(async {
        let root = Scratch::new().unwrap();
        std::fs::write(root.path().join("long.txt"), "row\n".repeat(2100)).unwrap();
        let runner = loaded(root.path()).await;
        let result = execute(&runner, "read", json!({"path":"long.txt"})).await;
        assert_eq!(result["details"]["truncation"]["outputLines"], 2000);
        assert_eq!(result["details"]["truncation"]["totalLines"], 2100);
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .ends_with("[Showing lines 1-2000 of 2101. Use offset=2001 to continue.]")
        );
        let registered = runner
            .get_all_registered_tools()
            .into_iter()
            .find(|tool| tool.definition.name() == "read")
            .unwrap();
        let tool = maestro_extensions::wrap_registered_tool(&registered, runner.clone());
        let signal = maestro_extensions::Cancellation::new();
        signal.abort();
        let error = (tool.execute)(
            "cancelled".into(),
            json!({"path":"long.txt"}),
            Some(signal),
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(error.message, "Operation aborted");
    });
}
/// Draws an independent literal using the same toolkit text layout.
fn expected(text: String) -> Vec<String> {
    use maestro_tui::Component as _;
    maestro_tui::Text::new(text, 0, 0, None).render(200)
}

/// Distinct supplied colors make semantic-role mistakes visible in assertions.
fn theme() -> maestro_theme::LiveTheme {
    use maestro_theme::{ColorMode, ColorValue, Theme, ThemeColor, ThemeOptions};
    let colors = [
        (ThemeColor::ToolTitle, ColorValue::Index(1)),
        (ThemeColor::Accent, ColorValue::Index(2)),
        (ThemeColor::ToolOutput, ColorValue::Index(3)),
        (ThemeColor::Warning, ColorValue::Index(4)),
        (ThemeColor::Muted, ColorValue::Index(5)),
        (ThemeColor::Error, ColorValue::Index(6)),
    ];
    maestro_theme::LiveTheme::from_theme(Rc::new(
        Theme::new(colors, [], ColorMode::Color256, ThemeOptions::default()).unwrap(),
    ))
}

/// Applies one literal semantic role using the native theme.
fn fg(role: &maestro_theme::ThemeColor, text: &str) -> String {
    theme().get().unwrap().fg(role, text).unwrap()
}

/// Call rows through the native renderer interface.
mod calls;

/// Expanded and collapsed results through the native renderer interface.
mod results;
