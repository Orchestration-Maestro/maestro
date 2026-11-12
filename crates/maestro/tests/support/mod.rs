//! Disposable configuration, a cleared executable environment and the shipped theme.
use maestro_test_directory::Scratch;
use maestro_test_process::{SearchGuard, install_stub_search_programs};
use std::{
    path::Path,
    process::{Command, Output},
    sync::OnceLock,
};

/// A root command with no ambient credentials or user settings. Its children find stub `fd` and
/// `rg` programs in the root's `bin` directory, and dropping the root fails the test when a
/// child ran a search program from the host instead.
pub struct Root {
    /// Owned temporary directories.
    pub scratch: Scratch,
    /// The guard on the command path, set by the first command.
    search: OnceLock<SearchGuard>,
}
impl Root {
    /// Create the user configuration root and project directory.
    pub fn new() -> Self {
        let scratch = Scratch::new().unwrap();
        for path in ["agent", "project"] {
            std::fs::create_dir(scratch.path().join(path)).unwrap();
        }
        share_component_cache(&scratch.path().join("agent"));
        Self {
            scratch,
            search: OnceLock::new(),
        }
    }
    /// The command path of the root's children: the stub search programs, then the host path,
    /// with the guard ahead of the first host directory.
    fn command_path(&self) -> &std::ffi::OsStr {
        self.search
            .get_or_init(|| {
                let bin = self.path("bin");
                install_stub_search_programs(&bin).unwrap();
                let host = std::env::var_os("PATH").unwrap_or_default();
                let path =
                    std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&host)))
                        .unwrap();
                SearchGuard::new(self.scratch.path(), &path).unwrap()
            })
            .path()
    }
    /// Root executable invocation with supplied literal arguments.
    pub fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_maestro"));
        command
            .env_clear()
            .current_dir(self.path("project"))
            .env("HOME", self.scratch.path())
            .env("PATH", self.command_path())
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

impl Drop for Root {
    fn drop(&mut self) {
        if let Some(search) = self.search.get()
            && !std::thread::panicking()
        {
            search.assert_unreached();
        }
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

/// The command path `root` gives its executable children.
#[cfg(unix)]
fn command_path(root: &Root) -> std::ffi::OsString {
    let command = root.command(&[]);
    command
        .get_envs()
        .find(|(key, _)| *key == "PATH")
        .and_then(|(_, value)| value.map(std::ffi::OsStr::to_owned))
        .unwrap()
}

#[cfg(unix)]
#[test]
fn root_command_resolves_fd_and_rg_inside_its_root() {
    let root = Root::new();
    let path = command_path(&root);
    for program in ["fd", "rg"] {
        let found = std::env::split_paths(&path)
            .find(|directory| directory.join(program).exists())
            .unwrap();
        assert!(
            found.starts_with(root.scratch.path()),
            "{program}: {found:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn root_fails_when_a_command_ran_a_host_search_program() {
    let root = Root::new();
    let path = command_path(&root);
    std::fs::remove_file(root.path("bin/rg")).unwrap();
    let ran = std::process::Command::new("rg").env("PATH", path).output();
    drop(ran);
    let ended = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(root)));
    assert!(ended.is_err(), "the root accepted a host search program");
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
pub fn widgets() -> std::rc::Rc<dyn maestro_extensions::ExtensionWidgets> {
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
