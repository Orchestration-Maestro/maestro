//! Engine files stay in their caller-created root for the instance's lifetime.
#![cfg(test)]
use maestro_extensions::{
    EventError, ExtensionRunner, ExtensionUIContext, Local, create_event_bus_for,
    create_extension_runtime, load_extension_from_factory,
    types::{
        CustomComponentFactory, CustomFactoryOutcome, CustomUIOptions, ExtensionUIContextPort,
        ExtensionUIFuture, SessionShutdownEvent,
    },
    widgets::ExtensionKeybindings,
};
use maestro_test_directory::Scratch;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    process::Command,
    rc::Rc,
    sync::RwLock,
};

/// UI that saves once through the actual engine input and then closes the view.
struct Ui {
    /// Real frame-writer capability passed to the factory.
    tui: maestro_tui::TUI,
    /// Parent of product-created temporary roots in this checked child.
    temporary: PathBuf,
    /// Root observed through its engine-written save.
    saved: Rc<RefCell<Option<PathBuf>>>,
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn custom_component(
        &self,
        factory: CustomComponentFactory,
        _: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        let (send, receive) = tokio::sync::oneshot::channel();
        let send = RefCell::new(Some(send));
        let close = Rc::new(move || {
            Ok(send
                .borrow_mut()
                .take()
                .is_some_and(|send| send.send(()).is_ok()))
        });
        let keys = ExtensionKeybindings {
            manager: maestro_tui::KeybindingsManager::new(Vec::new(), Vec::new()),
            reload: None,
        };
        let building = factory.call((self.tui.clone(), self.theme(), keys, close));
        let (temporary, saved) = (self.temporary.clone(), self.saved.clone());
        Box::pin(async move {
            let built = match building? {
                CustomFactoryOutcome::Ready(built) => built,
                CustomFactoryOutcome::Async(building) => building.await?,
            };
            let input = built.component.input_handler().unwrap();
            input
                .try_handle_input(" ")
                .map_err(|e| EventError::from(e.to_string()))?;
            let roots: Vec<_> = std::fs::read_dir(temporary)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.join("save.dat").is_file())
                .collect();
            assert_eq!(roots.len(), 1, "one engine root owns the save");
            assert_eq!(
                std::fs::read(roots[0].join("save.dat")).unwrap(),
                b"controlled WAD"
            );
            saved.replace(Some(roots[0].clone()));
            input
                .try_handle_input("q")
                .map_err(|e| EventError::from(e.to_string()))?;
            receive.await.map_err(|e| EventError::from(e.to_string()))?;
            if let Some(dispose) = built.dispose {
                dispose.call(())?;
            }
            Ok(())
        })
    }
}

/// Builds the player and a file-writing engine using the same public engine interface.
pub(super) fn component() -> PathBuf {
    let directory = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maestro-extensions-wasm"
    ));
    let player = super::extension_guide::cargo::component(
        directory,
        &["--example", "doom_overlay"],
        "doom_overlay",
    )
    .unwrap();
    let engine =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/doom-files-engine.wasm");
    let output = player
        .parent()
        .unwrap()
        .join(format!("doom-files-{}.wasm", std::process::id()));
    let result = Command::new("wasm-tools")
        .arg("compose")
        .arg(player)
        .arg("-d")
        .arg(engine)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    output
}

/// Records controlled files so unrelated host initialization cannot be attributed to the engine.
fn files(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    let mut result = Vec::new();
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            result.push((path.clone(), None));
            result.extend(files(&path));
        } else {
            result.push((path.clone(), Some(std::fs::read(path).unwrap())));
        }
    }
    result.sort();
    result
}

/// Runs with a private HOME, project and temporary parent, without mutating process globals.
#[test]
fn doom_overlay_files_are_private_and_deleted_on_shutdown() {
    checked(
        "doom_overlay_files_are_private_and_deleted_on_shutdown",
        false,
    );
}

/// A rejected initialization cannot leave its caller-created files behind.
#[test]
fn doom_overlay_failed_initialization_removes_private_files() {
    checked(
        "doom_overlay_failed_initialization_removes_private_files",
        true,
    );
}

/// Runs exactly one filesystem scenario in its own process.
fn checked(name: &str, empty: bool) {
    if !maestro_test_process::selection().is_child() {
        let scratch = Scratch::new().unwrap();
        for name in ["home", "project", "temporary"] {
            std::fs::create_dir(scratch.path().join(name)).unwrap();
        }
        let path = component();
        let status = maestro_test_process::TestProcess::current(name)
            .unwrap()
            .case(if empty { "empty" } else { "save" })
            .env("MAESTRO_DOOM_FILES_ROOT", scratch.path())
            .env("MAESTRO_DOOM_FILES_COMPONENT", path)
            .env("HOME", scratch.path().join("home"))
            .env("TMPDIR", scratch.path().join("temporary"))
            .env("TMP", scratch.path().join("temporary"))
            .env("TEMP", scratch.path().join("temporary"))
            .current_dir(scratch.path().join("project"))
            .nocapture()
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    super::wasi_support::run(run(empty));
}

/// Plays on the production loader and runner, then dispatches actual shutdown.
async fn run(empty: bool) {
    let root = PathBuf::from(std::env::var("MAESTRO_DOOM_FILES_ROOT").unwrap());
    let project = root.join("project");
    let wad = project.join("doom1.wad");
    let data = if empty { "" } else { "controlled WAD" };
    std::fs::write(&wad, data).unwrap();
    let tails = Rc::new(RefCell::new(Vec::new()));
    let started = tails.clone();
    let bus = create_event_bus_for::<Local>(
        Rc::new(move |tail| {
            started.borrow_mut().push(tokio::task::spawn_local(tail));
        }),
        Rc::new(|error| panic!("unexpected tail error: {error:?}")),
    );
    let runner = runner(&project, &bus).await;
    super::support::bind_prompt(&runner, "controlled game");
    let (tui, _terminal) = super::canonical_ui::screen();
    tui.start().unwrap();
    let saved = Rc::default();
    runner.set_ui_context(Some(ExtensionUIContext::new(Rc::new(Ui {
        tui: tui.clone(),
        temporary: root.join("temporary"),
        saved: Rc::clone(&saved),
    }))));
    let before_home = files(&root.join("home"));
    let before_project = files(&project);
    let handler = runner.get_command("doom-overlay").unwrap().command.handler;
    let result = handler(
        wad.to_str().unwrap().into(),
        runner.create_command_context(),
    )
    .await;
    assert_eq!(files(&root.join("home")), before_home);
    assert_eq!(files(&project), before_project);
    if empty {
        assert!(
            result.is_ok(),
            "a rejected startup is reported through the UI"
        );
        assert!(
            std::fs::read_dir(root.join("temporary"))
                .unwrap()
                .next()
                .is_none(),
            "failed initialization must delete its private root"
        );
        return;
    }
    result.unwrap();
    let private = saved.borrow().clone().unwrap();
    assert!(private.join("config.cfg").is_file());
    shutdown(&runner, &private).await;
    for tail in tails.take() {
        tail.await.unwrap();
    }
    tui.stop().unwrap();
}

/// Loads the player with the same factory and session services as the native host.
async fn runner(
    project: &Path,
    bus: &maestro_extensions::EventBusController<Local>,
) -> ExtensionRunner<Local> {
    let runtime = create_extension_runtime::<Local>();
    let path = std::env::var("MAESTRO_DOOM_FILES_COMPONENT").unwrap();
    let owner = load_extension_from_factory(
        crate::artifact(path.clone()),
        project.to_str().unwrap().into(),
        bus.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        super::session::runner_session(project.to_str().unwrap()).0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    )
}

/// Reproduces committed bytes when the optional native compiler is configured.
#[test]
fn doom_files_engine_fixture_matches_source() {
    let result = Command::new("python3")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/regenerate_doom_files.py"))
        .arg("--check")
        .output()
        .unwrap();
    print!("{}", String::from_utf8_lossy(&result.stdout));
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

/// Completes actual session shutdown before checking the retained root.
async fn shutdown(runner: &ExtensionRunner<Local>, private: &Path) {
    let mut event: SessionShutdownEvent =
        serde_json::from_value(serde_json::json!({"reason":"quit"})).unwrap();
    runner.emit(&mut event).await.unwrap();
    assert!(
        !private.exists(),
        "shutdown must delete the private engine root"
    );
}
