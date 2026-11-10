//! Disposable configuration, a cleared executable environment and the shipped theme.
use maestro_test_directory::Scratch;
use std::{
    path::Path,
    process::{Command, Output},
};

/// A root command with no ambient credentials or user settings.
pub struct Root {
    /// Owned temporary directories.
    pub scratch: Scratch,
}
impl Root {
    /// Create the user configuration root and project directory.
    pub fn new() -> Self {
        let scratch = Scratch::new().unwrap();
        for path in ["agent", "project"] {
            std::fs::create_dir(scratch.path().join(path)).unwrap();
        }
        share_component_cache(&scratch.path().join("agent"));
        Self { scratch }
    }
    /// Root executable invocation with supplied literal arguments.
    pub fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_maestro"));
        command
            .env_clear()
            .current_dir(self.path("project"))
            .env("HOME", self.scratch.path())
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            // A Windows process without its system root cannot open sockets.
            .envs(std::env::var_os("SYSTEMROOT").map(|root| ("SYSTEMROOT", root)))
            .env("MAESTRO_CODING_AGENT_DIR", self.path("agent"))
            .env(
                "MAESTRO_PACKAGE_DIR",
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap(),
            )
            .env("MAESTRO_OFFLINE", "1")
            .env("NO_COLOR", "1")
            .args(args);
        command
    }
    /// Run a root command to completion and return both streams.
    pub fn output(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }
    /// A path under the disposable root.
    pub fn path(&self, path: &str) -> std::path::PathBuf {
        self.scratch.path().join(path)
    }
}

/// Links the compiled components of the run's agent directory into the component cache of
/// `agent`, so an executable that uses `agent` loads the components the run prepared instead of
/// compiling them again. Does nothing when the run exported no agent directory.
pub fn share_component_cache(agent: &Path) {
    if let Some(run) = maestro_test_process::run_agent_directory() {
        maestro_test_process::link_files(
            &maestro_extensions_wasmtime::component_cache_directory(run),
            &maestro_extensions_wasmtime::component_cache_directory(agent),
        )
        .unwrap();
    }
}

/// Stops and reaps a borrowed child if its caller unwinds before normal cleanup.
pub struct ChildGuard<'a>(pub &'a mut std::process::Child);

impl Drop for ChildGuard<'_> {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn child_guard_reaps_during_unwind() {
    let root = Root::new();
    let mut child = root
        .command(&["--mode", "rpc"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = ChildGuard(&mut child);
        panic!("simulated selector read failure");
    }));
    let status = child.try_wait().unwrap();
    let _ = child.kill();
    child.wait().unwrap();
    assert!(unwind.is_err());
    assert!(status.is_some(), "child survived selector failure");
}

#[test]
fn root_agent_reuses_the_component_cache_the_run_prepared() {
    let run = Scratch::new().unwrap();
    let text = run.path().to_str().unwrap().to_owned();
    let selected = maestro_test_process::run_per_environment(
        "root_agent_reuses_the_component_cache_the_run_prepared",
        [text],
        |run| {
            std::collections::BTreeMap::from([(
                maestro_test_process::AGENT_DIRECTORY,
                Some(run.clone()),
            )])
        },
    )
    .unwrap();
    if selected.is_some() {
        let run = maestro_test_process::run_agent_directory().unwrap();
        let prepared =
            maestro_extensions_wasmtime::component_cache_directory(&run).join("modules/namespace");
        std::fs::create_dir_all(&prepared).unwrap();
        std::fs::write(prepared.join("entry"), b"compiled").unwrap();
        let root = Root::new();
        let shared = maestro_extensions_wasmtime::component_cache_directory(
            root.path("agent").to_str().unwrap(),
        )
        .join("modules/namespace/entry");
        assert_eq!(std::fs::read(shared).unwrap(), b"compiled");
    }
}

#[cfg(unix)]
pub mod native;
pub mod rpc;

impl Root {
    /// Configure one controlled model without ambient provider credentials.
    pub fn models(&self, base: &str) {
        std::fs::write(self.path("agent/models.json"), serde_json::json!({"providers": {"startup": {
            "baseUrl": base, "apiKey": "controlled", "api": "openai-completions",
            "models": [{"id": "controlled", "reasoning": true, "contextWindow": 200_000, "maxTokens":8192}],
        }}}).to_string()).unwrap();
    }
}
pub mod provider;
pub mod shipped_stdout;

/// Run one root protocol case against the controlled HTTP provider.
pub fn connected(test: impl FnOnce(&Root, &mut rpc::Rpc, &provider::Provider)) {
    let root = Root::new();
    let provider = provider::Provider::new();
    root.models(&provider.base);
    let mut rpc = rpc::Rpc::start(&root);
    test(&root, &mut rpc, &provider);
    rpc.finish();
}

/// Read the single persisted history named by the running server.
pub fn history(rpc: &mut rpc::Rpc) -> Vec<serde_json::Value> {
    let state = rpc.request(serde_json::json!({"type":"get_state"}));
    let text = std::fs::read_to_string(state["sessionFile"].as_str().unwrap()).unwrap();
    text.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

impl Root {
    /// A supplied native export bundle using the export owner's actual HTML template.
    pub fn bundle(&self) -> std::path::PathBuf {
        let root = self.path("bundle");
        let export = root.join("dist/core/export-html");
        std::fs::create_dir_all(export.join("vendor")).unwrap();
        std::fs::write(
            export.join("template.html"),
            include_str!("../../../maestro-export/assets/document/template.html"),
        )
        .unwrap();
        for name in [
            "template.css",
            "template.js",
            "vendor/marked.min.js",
            "vendor/highlight.min.js",
        ] {
            std::fs::write(export.join(name), "").unwrap();
        }
        let themes = root.join("crates/maestro-theme/assets/theme");
        std::fs::create_dir_all(&themes).unwrap();
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("maestro-theme/assets/theme");
        for name in ["dark.json", "light.json"] {
            std::fs::copy(source.join(name), themes.join(name)).unwrap();
        }
        root
    }
}
pub mod screen;

/// Install authored component bytes and their three dynamic-resource collections.
#[cfg(target_os = "linux")]
pub fn resources(root: &Root) -> std::path::PathBuf {
    let base = root.path("project/extensions");
    std::fs::create_dir_all(&base).unwrap();
    std::fs::copy(
        std::env::var_os("MAESTRO_RESOURCE_COMPONENT").expect("nextest author-component setup"),
        base.join("index.wasm"),
    )
    .unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("maestro-theme/assets/theme/dark.json");
    let theme: serde_json::Value = serde_json::from_slice(&std::fs::read(source).unwrap()).unwrap();
    for version in ["one", "two"] {
        let directory = base.join(version);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("SKILL.md"), format!("---\nname: resource-{version}\ndescription: Controlled resource {version}\n---\nResource {version}\n")).unwrap();
        std::fs::write(
            directory.join("dynamic.md"),
            format!(
                "---\ndescription: Resource prompt {version}\n---\nResource prompt {version}\n"
            ),
        )
        .unwrap();
        let mut theme = theme.clone();
        theme["name"] = format!("resource-{version}-theme").into();
        std::fs::write(directory.join("dynamic.json"), theme.to_string()).unwrap();
    }
    base
}

/// The application's host widgets over the default live theme.
pub fn widgets() -> std::rc::Rc<dyn maestro_app::ExtensionWidgets> {
    let highlighter = std::rc::Rc::new(
        maestro_theme::SyntectHighlighter::new().expect("bundled syntax definitions"),
    );
    std::rc::Rc::new(maestro_chat::ExtensionWidgetHost::new(
        maestro_theme::LiveTheme::default(),
        highlighter,
        maestro_tui::TerminalImage::default(),
    ))
}

/// SDK construction for compiled extension witnesses.
pub mod sdk;

/// A prepared shipped theme for loader decoration, editor styling and the example renderers.
pub fn theme() -> maestro_theme::LiveTheme {
    let theme = maestro_theme::load_theme_from_path(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../maestro-theme/assets/theme/dark.json"
        ),
        Some(maestro_theme::ColorMode::Truecolor),
    )
    .unwrap();
    maestro_theme::LiveTheme::from_theme(std::rc::Rc::new(theme))
}
