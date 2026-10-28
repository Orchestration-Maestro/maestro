//! WAD discovery and controlled download through the shipped command.
#![cfg(test)]
use maestro_extensions::{
    EventBusController, ExtensionExecution, ExtensionRunner, ExtensionUIContext, Local,
    create_event_bus_for, create_extension_runtime, load_extension_from_factory,
};
use maestro_test_directory::Scratch;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::RwLock,
};
mod io;
mod reload;
mod ui;

/// One loaded instance and its caller-owned effects.
struct Demo {
    /// Command and lifecycle dispatcher.
    runner: ExtensionRunner<Local>,
    /// Keeps the background channel alive.
    _bus: EventBusController<Local>,
    /// Actual announced background jobs.
    tails: Rc<RefCell<Vec<tokio::task::JoinHandle<()>>>>,
    /// Notices and the WAD observed by the engine.
    ui: Rc<ui::Ui>,
    /// Original session capabilities reused by a fresh component instance.
    session: maestro_extensions::runner::ExtensionRunnerSession,
    /// Writable history kept by the application owner.
    history: Rc<RefCell<maestro_session::SessionManager>>,
    /// Controlled HTTP request owner.
    io: Rc<io::Io>,
}

/// Loads from a controlled module directory with real filesystem access.
async fn loaded(root: &Path, case: &str) -> Demo {
    let (session, history) = super::session::runner_session(root.join("project").to_str().unwrap());
    loaded_on(root, case, session, history).await
}

/// Loads a fresh component while preserving the caller's session history.
async fn loaded_on(
    root: &Path,
    case: &str,
    session: maestro_extensions::runner::ExtensionRunnerSession,
    history: Rc<RefCell<maestro_session::SessionManager>>,
) -> Demo {
    let tails = Rc::new(RefCell::new(Vec::new()));
    let started = tails.clone();
    let bus = create_event_bus_for::<Local>(
        Rc::new(move |tail| {
            started.borrow_mut().push(tokio::task::spawn_local(tail));
        }),
        Rc::new(|error| panic!("unexpected background error: {error:?}")),
    );
    let io = controlled_io(case);
    let runner = runner(root, &bus, io.clone(), &session).await;
    bind_history(&runner, history.clone());
    let (tui, _terminal) = super::canonical_ui::screen();
    let ui = Rc::new(ui::Ui {
        tui,
        temporary: root.join("temporary"),
        notices: RefCell::default(),
        wad: Rc::default(),
    });
    if case != "headless" {
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
    }
    Demo {
        runner,
        _bus: bus,
        session,
        history,
        tails,
        ui,
        io,
    }
}

/// Native candidate locations in their documented order.
fn candidates(root: &Path) -> [PathBuf; 5] {
    [
        root.join("module/doom1.wad"),
        root.join("project/doom1.wad"),
        root.join("project/DOOM1.WAD"),
        root.join("home/doom1.wad"),
        root.join("home/.doom/doom1.wad"),
    ]
}

/// Seeds lower-priority choices too, so the selected bytes prove ordering.
fn prepare(root: &Path, case: &str) -> (String, Option<Vec<u8>>) {
    if let Ok(first) = case.parse::<usize>() {
        for (index, path) in candidates(root).iter().enumerate().skip(first) {
            if !path.exists() {
                std::fs::write(path, format!("candidate {index}")).unwrap();
            }
        }
        return (
            String::new(),
            Some(format!("candidate {first}").into_bytes()),
        );
    }
    match case {
        "explicit" => {
            let path = root.join("custom.wad");
            std::fs::write(&path, b"custom WAD").unwrap();
            (path.to_str().unwrap().into(), Some(b"custom WAD".to_vec()))
        }
        "download" => (String::new(), Some(b"controlled WAD".to_vec())),
        "missing" => ("missing.wad".into(), None),
        _ => (String::new(), None),
    }
}

/// Checks the selected bytes or a visible failure without an opened game.
fn observed(demo: &Demo, root: &Path, case: &str, expected: Option<&[u8]>) {
    assert_eq!(demo.ui.wad.borrow().as_deref(), expected);
    if expected.is_some() {
        assert!(
            demo.ui
                .notices
                .borrow()
                .iter()
                .any(|message| message == "Loading DOOM...")
        );
    }
    if ["download", "http-error", "stream-error"].contains(&case) {
        assert_eq!(
            *demo.io.requests.borrow(),
            ["https://distro.ibiblio.org/slitaz/sources/packages/d/doom1.wad"]
        );
        if case == "download" {
            assert_eq!(
                std::fs::read(root.join("module/doom1.wad")).unwrap(),
                b"controlled WAD"
            );
        } else {
            assert!(!root.join("module/doom1.wad").exists());
        }
    } else {
        assert!(demo.io.requests.borrow().is_empty());
    }
    if ["missing", "http-error", "stream-error"].contains(&case) {
        assert!(
            demo.ui
                .notices
                .borrow()
                .iter()
                .any(|message| message.starts_with("Failed to"))
        );
        assert!(demo.ui.wad.borrow().is_none());
    }
}

/// Runs one environment-isolated scenario without changing the parent's HOME.
async fn run() {
    let root = PathBuf::from(std::env::var("MAESTRO_DOOM_WAD_ROOT").unwrap());
    let case = std::env::var("MAESTRO_DOOM_WAD_CASE").unwrap();
    if case == "switch" {
        reload::switch(&root).await;
        return;
    }
    let (args, expected) = prepare(&root, &case);
    let demo = loaded(&root, &case).await;
    (demo
        .runner
        .get_command("doom-overlay")
        .unwrap()
        .command
        .handler)(args, demo.runner.create_command_context())
    .await
    .unwrap();
    observed(&demo, &root, &case, expected.as_deref());
    let mut shutdown: maestro_extensions::types::SessionShutdownEvent =
        serde_json::from_value(serde_json::json!({"reason":"quit"})).unwrap();
    demo.runner.emit(&mut shutdown).await.unwrap();
    for tail in demo.tails.take() {
        tail.await.unwrap();
    }
    assert!(
        std::fs::read_dir(root.join("temporary"))
            .unwrap()
            .next()
            .is_none()
    );
}

/// Builds once, then executes each scenario in an exclusive disposable root.
fn checked(name: &str, cases: &[&str]) {
    if maestro_test_process::selection().is_child() {
        super::wasi_support::run(run());
        return;
    }
    let component = super::doom_overlay_files::component();
    for case in cases {
        let scratch = Scratch::new().unwrap();
        for name in ["module", "project", "home/.doom", "temporary"] {
            std::fs::create_dir_all(scratch.path().join(name)).unwrap();
        }
        std::fs::copy(&component, scratch.path().join("module/doom_overlay.wasm")).unwrap();
        let status = maestro_test_process::TestProcess::current(name)
            .unwrap()
            .case(case)
            .env("MAESTRO_DOOM_WAD_ROOT", scratch.path())
            .env("MAESTRO_DOOM_WAD_CASE", case)
            .env("HOME", scratch.path().join("home"))
            .env("USERPROFILE", scratch.path().join("home"))
            .env("TMPDIR", scratch.path().join("temporary"))
            .env("TMP", scratch.path().join("temporary"))
            .env("TEMP", scratch.path().join("temporary"))
            .current_dir(scratch.path().join("project"))
            .nocapture()
            .status()
            .unwrap();
        assert!(status.success(), "scenario {case}");
    }
}

/// Module, working-directory and home candidates keep their priority.
#[test]
fn doom_wad_search_uses_existing_files_in_order() {
    checked(
        "doom_wad_search_uses_existing_files_in_order",
        &["0", "1", "2", "3", "4"],
    );
}
/// An explicit path bypasses discovery and download.
#[test]
fn doom_wad_explicit_path_overrides_discovery() {
    checked("doom_wad_explicit_path_overrides_discovery", &["explicit"]);
}
/// Native chunks are collected as bytes and cached beside the module.
#[test]
fn doom_wad_downloads_and_caches_raw_chunks() {
    checked("doom_wad_downloads_and_caches_raw_chunks", &["download"]);
}
/// HTTP and body failures leave no cache or opened view.
#[test]
fn doom_wad_failures_notify_without_partial_cache() {
    checked(
        "doom_wad_failures_notify_without_partial_cache",
        &["missing", "http-error", "stream-error"],
    );
}
/// Headless invocation performs no filesystem-dependent startup or request.
#[test]
fn doom_wad_headless_command_does_not_start_engine() {
    checked(
        "doom_wad_headless_command_does_not_start_engine",
        &["headless"],
    );
}

/// Loads one component with the native effect owner kept outside its store.
async fn runner(
    root: &Path,
    bus: &EventBusController<Local>,
    io: Rc<io::Io>,
    session: &maestro_extensions::runner::ExtensionRunnerSession,
) -> ExtensionRunner<Local> {
    let path = root
        .join("module/doom_overlay.wasm")
        .to_str()
        .unwrap()
        .to_owned();
    let runtime = create_extension_runtime::<Local>();
    let owner = load_extension_from_factory(
        maestro_extensions_wasmtime::artifact_factory_with_io(
            path.clone(),
            super::support::widgets(),
            maestro_extensions_wasmtime::Installation {
                version: "wad-host".into(),
                agent_dir: root.join("agent").to_str().unwrap().into(),
            },
            io,
        ),
        root.join("project").to_str().unwrap().into(),
        bus.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    let runner = ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        maestro_extensions::runner::ExtensionRunnerSession {
            cwd: session.cwd.clone(),
            session_manager: session.session_manager.clone(),
            model_registry: session.model_registry.clone(),
        },
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    super::support::bind_prompt(&runner, "controlled WAD");
    runner
}

/// Supplies deterministic chunks or a failure after their prefix.
fn controlled_io(case: &str) -> Rc<io::Io> {
    let chunks = [
        Ok(b"con".to_vec()),
        Ok(b"trolled".to_vec()),
        if case == "stream-error" {
            Err("controlled body failure".into())
        } else {
            Ok(b" WAD".to_vec())
        },
    ];
    Rc::new(io::Io {
        requests: RefCell::default(),
        response: Rc::new(io::Reply {
            status: if case == "http-error" { 503 } else { 200 },
            reader: Rc::new(io::Reader {
                chunks: RefCell::new(chunks.into()),
            }),
        }),
    })
}

/// WAD switching plays once after a fresh instance loads, never on later resume.
#[test]
fn doom_wad_switch_reloads_and_consumes_pending_play_once() {
    checked(
        "doom_wad_switch_reloads_and_consumes_pending_play_once",
        &["switch"],
    );
}

/// Binds hidden custom-entry writes to the same in-memory history the runner reads.
pub(super) fn bind_history(
    runner: &ExtensionRunner<Local>,
    history: Rc<std::cell::RefCell<maestro_session::SessionManager>>,
) {
    let actions: maestro_extensions::ExtensionActions<Local> =
        maestro_extensions::ExtensionActions {
            append_entry: Rc::new(move |kind: String, data: Option<serde_json::Value>| {
                history
                    .borrow_mut()
                    .append_custom_entry(
                        &kind,
                        data.map_or(
                            maestro_session::Presence::Missing,
                            maestro_session::Presence::Present,
                        ),
                    )
                    .map(|_| ())
                    .map_err(|error| error.to_string().into())
            }),
            ..Local::unbound_actions()
        };
    runner
        .bind_core(
            actions,
            maestro_extensions::ExtensionContextActions::default(),
            maestro_extensions::types::ProviderActions::default(),
        )
        .unwrap();
}
