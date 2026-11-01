//! The shipped border status editor on the production runner: the editor its session start
//! installs, the status it writes into the borders and the branch it reads with Git.
#![cfg(test)]
use maestro_extensions::{
    ContextUsage, EventBusController, EventError, ExtensionContextActions, ExtensionExecution,
    ExtensionRunner, ExtensionUIContext, Local, create_event_bus_for, create_extension_runtime,
    load_extension_from_factory,
    types::{
        EditorFactory, EditorFactoryOutcome, ExtensionKeybindings, ExtensionUIContextPort,
        FooterFactory, ProviderActions,
    },
};
use maestro_test_directory::Scratch;
use maestro_theme::{LiveTheme, Theme, ThemeColor};
use maestro_tui::{TUI, TerminalImage};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    path::Path,
    process::Command,
    rc::Rc,
    sync::{Arc, RwLock},
};
use tokio::task::JoinHandle;

use super::{manual_runtime::ManualRuntime, recording_terminal::RecordingTerminal};

/// The environment variable naming the built component.
const COMPONENT: &str = "MAESTRO_BORDER_STATUS_EDITOR_COMPONENT";
/// A directory that does not exist, so its branch read fails.
const MISSING: &str = "/border-status/project";
/// The bottom-left status for the supplied model and thinking level.
const LEFT: &str = " fixture/m · high ";
/// The bottom-right status for the supplied estimate in the missing directory.
const STATUS: &str = " ctx 12%/200k · /border-status/project ";

/// The application's editor, footer and working-row slots.
struct Ui {
    /// The terminal factories receive.
    tui: TUI,
    /// The published dark theme.
    theme: LiveTheme,
    /// The editor the current factory built.
    built: RefCell<Option<EditorFactoryOutcome>>,
    /// The footer factory the session set.
    footer: RefCell<Option<FooterFactory>>,
    /// The last working-row visibility the session set.
    working: Cell<Option<bool>>,
}

impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> LiveTheme {
        self.theme.clone()
    }
    fn set_working_visible(&self, visible: bool) -> Result<(), EventError> {
        self.working.set(Some(visible));
        Ok(())
    }
    fn set_footer(&self, factory: Option<FooterFactory>) -> Result<(), EventError> {
        self.footer.replace(factory);
        Ok(())
    }
    fn set_editor_component(&self, factory: Option<EditorFactory>) -> Result<(), EventError> {
        let keybindings = ExtensionKeybindings {
            manager: maestro_tui::KeybindingsManager::new(
                maestro_tui::TUI_KEYBINDINGS.clone(),
                Vec::new(),
            ),
            reload: None,
        };
        let theme = maestro_theme::get_editor_theme(self.theme());
        let built = factory
            .map(|factory| factory.call((self.tui.clone(), theme, keybindings)))
            .transpose()?;
        self.built.replace(built);
        Ok(())
    }
}

/// The built example on the production runner, its UI and the work it handed to the host.
struct Harness {
    /// Production runner over the compiled shipped example.
    runner: ExtensionRunner<Local>,
    /// Keeps the example's channel listener registered.
    _bus: EventBusController<Local>,
    /// The application slots.
    ui: Rc<Ui>,
    /// The terminal runtime whose scheduled work the test runs.
    screen: ManualRuntime,
    /// Background work handed to the host, in announcement order.
    tails: Rc<RefCell<Vec<Option<JoinHandle<()>>>>>,
    /// The supplied context estimate.
    usage: Rc<Cell<Option<ContextUsage>>>,
    /// The supplied model.
    model: Rc<RefCell<Option<Arc<RwLock<maestro_models::Model>>>>>,
    /// The dark theme the expected rows are painted with.
    theme: Rc<Theme>,
}

impl Harness {
    /// Loads the compiled example with `cwd` as the session directory.
    async fn new(cwd: &str) -> Self {
        let (theme, live) = dark();
        let tails: Rc<RefCell<Vec<Option<JoinHandle<()>>>>> = Rc::default();
        let spawned = Rc::clone(&tails);
        let bus = create_event_bus_for::<Local>(
            Rc::new(move |tail| {
                let handle = tokio::task::spawn_local(tail);
                spawned.borrow_mut().push(Some(handle));
            }),
            Rc::new(|error| panic!("unexpected tail error: {error:?}")),
        );
        let path = std::env::var(COMPONENT).expect("nextest component setup");
        let runtime = create_extension_runtime::<Local>();
        let factory = maestro_extensions_wasmtime::artifact_factory(
            path.clone(),
            super::support::widgets_over(live.clone()),
            maestro_extensions_wasmtime::Installation {
                version: "border-status-host".into(),
                agent_dir: "/controlled-agent".into(),
            },
        );
        let owner = load_extension_from_factory(
            factory,
            cwd.into(),
            bus.for_extension(),
            runtime.clone(),
            Some(path),
        )
        .await
        .unwrap();
        let runner = ExtensionRunner::<Local>::new(
            Rc::new(RwLock::new(vec![owner])),
            runtime,
            super::session::runner_session(cwd).0,
            Rc::new(|_| {}),
            live.clone(),
        );
        let (tui, screen) = screen();
        let ui = Rc::new(Ui {
            tui,
            theme: live,
            built: RefCell::default(),
            footer: RefCell::default(),
            working: Cell::default(),
        });
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        let harness = Self {
            runner,
            _bus: bus,
            ui,
            screen,
            tails,
            usage: Rc::new(Cell::new(Some(usage(Some(12.4), 200_000.0)))),
            model: Rc::new(RefCell::new(Some(super::model_support::model("m")))),
            theme,
        };
        harness.bind();
        harness
    }

    /// Binds the live model, context estimate and a high thinking level.
    fn bind(&self) {
        let mut actions = Local::unbound_actions();
        actions.get_thinking_level = Rc::new(|| Ok(maestro_models::ModelThinkingLevel::High));
        let (model, usage) = (Rc::clone(&self.model), Rc::clone(&self.usage));
        let context = ExtensionContextActions {
            get_model: Rc::new(move || Ok(model.borrow().clone())),
            get_context_usage: Rc::new(move || Ok(usage.get())),
            ..Default::default()
        };
        self.runner
            .bind_core(actions, context, ProviderActions::default())
            .unwrap();
        let _remove = self.runner.on_error(Rc::new(|error| {
            panic!("unexpected callback error: {error:?}")
        }));
    }

    /// Delivers one lifecycle event built from its JSON form.
    async fn emit(&self, name: &str) {
        match name {
            "session_start" => {
                let mut event: maestro_extensions::SessionStartEvent =
                    serde_json::from_value(json!({"reason": "startup"})).unwrap();
                assert!(self.runner.emit(&mut event).await.unwrap().is_none());
            }
            "agent_start" => {
                let mut event = maestro_extensions::AgentStartEvent {};
                assert!(self.runner.emit(&mut event).await.unwrap().is_none());
            }
            "agent_end" => {
                let mut event: maestro_extensions::types::AgentEndEvent =
                    serde_json::from_value(json!({"messages": []})).unwrap();
                self.runner.emit(&mut event).await.unwrap();
            }
            _ => {
                let mut event: maestro_extensions::types::SessionShutdownEvent =
                    serde_json::from_value(json!({"reason": "quit"})).unwrap();
                self.runner.emit(&mut event).await.unwrap();
            }
        }
    }

    /// Starts the session and waits for its branch read to finish.
    async fn start(&self) {
        self.emit("session_start").await;
        assert_eq!(self.tails.borrow().len(), 1, "one branch read");
        self.finish(0).await;
        self.screen.settle().unwrap();
    }

    /// Waits for the work announced `index`-th to end.
    async fn finish(&self, index: usize) {
        let handle = self.tails.borrow_mut()[index].take().unwrap();
        handle.await.unwrap();
    }

    /// The rows the installed editor renders `width` cells wide.
    fn rows(&self, width: usize) -> Vec<String> {
        let built = self.ui.built.borrow().clone().expect("installed editor");
        built.editor().try_render(width).unwrap()
    }

    /// Text painted in a theme color.
    fn paint(&self, color: &ThemeColor, text: &str) -> String {
        maestro_theme::paint(Some(&self.theme), color, text)
    }

    /// Muted text cut short, as the terminal helper ends a clipped styled text.
    fn cut(&self, text: &str) -> String {
        let muted = self.theme.get_fg_ansi(&ThemeColor::Muted).unwrap();
        format!("{muted}{text}\x1b[0m")
    }

    /// A border row: corners, the left text, `gap` fill cells and the right text.
    fn border(&self, left: &str, gap: usize, right: &str) -> String {
        let paint = maestro_theme::get_editor_theme(self.ui.theme.clone()).border_color;
        let corner = paint("─");
        format!("{corner}{left}{}{right}{corner}", paint(&"─".repeat(gap)))
    }

    /// The bottom row `width` cells wide whose plain sides fit with room to spare.
    fn status(&self, width: usize, right: &str) -> String {
        let gap = width - 2 - LEFT.chars().count() - right.chars().count();
        let muted = |text| self.paint(&ThemeColor::Muted, text);
        self.border(&muted(LEFT), gap, &muted(right))
    }
}

/// A context estimate with a known window.
fn usage(percent: Option<f64>, context_window: f64) -> ContextUsage {
    ContextUsage {
        tokens: None,
        context_window,
        percent,
    }
}

/// The application's shipped dark theme, as an instance and published live.
fn dark() -> (Rc<Theme>, LiveTheme) {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maestro-theme/assets/theme/dark.json"
    );
    let theme =
        maestro_theme::load_theme_from_path(path, Some(maestro_theme::ColorMode::Truecolor));
    let theme = Rc::new(theme.unwrap());
    (Rc::clone(&theme), LiveTheme::from_theme(theme))
}

/// A started frame writer on a manual runtime whose scheduled work the test runs.
fn screen() -> (TUI, ManualRuntime) {
    let terminal = RecordingTerminal::new(80, 24);
    let runtime = ManualRuntime::new();
    let tui = TUI::new(
        terminal.handle(),
        runtime.handle(),
        TerminalImage::default(),
        Some(false),
    );
    tui.start().unwrap();
    runtime.settle().unwrap();
    (tui, runtime)
}

/// Runs on a local executor.
fn run(action: impl Future<Output = ()>) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, action);
}

/// Session start hides the working row, installs an empty footer and an editor whose bottom
/// border carries the model, thinking level, context estimate and directory.
#[test]
fn border_status_editor_component_replaces_footer_and_working_row() {
    run(async {
        let h = Harness::new(MISSING).await;
        h.start().await;
        assert_eq!(h.ui.working.get(), Some(false));
        let footer = h.ui.footer.borrow().clone().expect("a footer");
        let metadata: Rc<dyn maestro_extensions::footer_data_provider::ReadonlyFooterDataProvider> =
            Rc::new(super::canonical_ui::Metadata::default());
        let footer = footer.call((h.ui.tui.clone(), h.ui.theme.clone(), metadata));
        assert!(footer.unwrap().component.try_render(80).unwrap().is_empty());
        let rows = h.rows(80);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], h.border("", 78, ""));
        assert_eq!(rows[2], h.status(80, STATUS));
        assert!(
            h.built().as_custom().is_some(),
            "the application's actions stay"
        );
    });
}

impl Harness {
    /// The editor the current factory built.
    fn built(&self) -> EditorFactoryOutcome {
        self.ui.built.borrow().clone().unwrap()
    }
}

/// The right text gives way first, then the left; three fill cells and two corners stay.
#[test]
fn border_status_editor_component_fits_narrow_borders() {
    run(async {
        let h = Harness::new(MISSING).await;
        h.start().await;
        let muted = h.paint(&ThemeColor::Muted, LEFT);
        let paint = maestro_theme::get_editor_theme(h.ui.theme.clone()).border_color;
        for (width, bottom) in [
            (40, h.border(&muted, 3, &h.cut(" ctx 12%/200k · /"))),
            (20, h.border(&h.cut(" fixture/m · hi"), 3, "")),
            (5, h.border("", 3, "")),
            (2, h.border("", 0, "")),
            (1, paint("─")),
            (0, String::new()),
        ] {
            let rows = h.rows(width);
            assert_eq!(rows.last().unwrap(), &bottom, "{width} cells");
            let top = match width {
                0 | 1 => bottom.clone(),
                _ => h.border("", width - 2, ""),
            };
            assert_eq!(rows[0], top, "{width} cells");
        }
    });
}

/// The estimate rounds the percentage and the window in thousands, ties upward, and shows
/// `?` without a percentage or a window.
#[test]
fn border_status_editor_component_formats_context_estimates() {
    run(async {
        let h = Harness::new(MISSING).await;
        h.start().await;
        for (estimate, shown) in [
            (Some(usage(Some(12.5), 200_500.0)), "ctx 13%/201k"),
            (Some(usage(Some(0.4), 128_000.0)), "ctx 0%/128k"),
            (Some(usage(Some(99.5), 1_047_576.0)), "ctx 100%/1048k"),
            (Some(usage(None, 200_000.0)), "ctx ?"),
            (Some(usage(Some(50.0), 0.0)), "ctx ?"),
            (None, "ctx ?"),
        ] {
            h.usage.set(estimate);
            let right = format!(" {shown} · {MISSING} ");
            assert_eq!(h.rows(80)[2], h.status(80, &right), "{estimate:?}");
        }
        h.model.replace(None);
        let row = h.rows(80).pop().unwrap();
        assert!(row.contains(" no model · high "), "{row:?}");
    });
}

/// Runs Git in `root` with settings that keep the developer's configuration out.
fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "commit.gpgsign=false", "-c", "core.hooksPath="])
        .args(args)
        .current_dir(root)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

/// The branch Git reports appears after the directory; a directory outside a repository or
/// one that does not exist shows none, without an error.
#[test]
fn border_status_editor_component_reads_the_branch_once() {
    run(async {
        let repository = Scratch::new().unwrap();
        git(repository.path(), &["init", "-q", "-b", "main"]);
        let plain = Scratch::new().unwrap();
        for (cwd, branch) in [
            (plain.path().to_str().unwrap(), false),
            (MISSING, false),
            (repository.path().to_str().unwrap(), true),
        ] {
            let h = Harness::new(cwd).await;
            h.start().await;
            let row = h.rows(200).pop().unwrap();
            assert_eq!(row.contains(" ("), branch, "{cwd}: {row:?}");
            assert_eq!(row.contains(" (main) "), branch, "{cwd}: {row:?}");
            if branch {
                git(
                    repository.path(),
                    &["symbolic-ref", "HEAD", "refs/heads/topic"],
                );
                assert_eq!(h.rows(200).pop().unwrap(), row, "read once per session");
            }
        }
    });
}

/// Two slash commands, offered for any text that starts a command.
struct Commands;

impl maestro_tui::AutocompleteProvider for Commands {
    type Signal = maestro_tui::Cancellation;
    fn get_suggestions<'a>(
        &'a self,
        _: &'a [String],
        _: maestro_tui::autocomplete::CursorPosition,
        _: maestro_tui::autocomplete::CompletionOptions<'a, maestro_tui::Cancellation>,
    ) -> super::editor_factories::Pending<'a> {
        let item = |name: &str| maestro_tui::AutocompleteItem {
            value: format!("/{name}"),
            label: name.to_owned(),
            description: None,
        };
        Box::pin(std::future::ready(Ok(Some(
            maestro_tui::AutocompleteSuggestions {
                items: vec![item("first"), item("second")],
                prefix: "/".to_owned(),
            },
        ))))
    }
    fn apply_completion(
        &self,
        lines: &[String],
        cursor: maestro_tui::autocomplete::CursorPosition,
        _: &maestro_tui::AutocompleteItem,
        _: &str,
    ) -> maestro_tui::autocomplete::CompletionResult {
        maestro_tui::autocomplete::CompletionResult {
            lines: lines.to_vec(),
            cursor_line: cursor.line,
            cursor_col: cursor.col,
        }
    }
}

/// While completions show, every suggestion stays visible below the plain bottom border; the
/// status returns once the list closes.
#[test]
fn border_status_editor_component_keeps_completion_rows() {
    run(async {
        let h = Harness::new(MISSING).await;
        h.start().await;
        let editor = h.built().editor();
        let provider = Rc::new(Commands);
        assert_eq!(
            editor.try_set_autocomplete_provider(provider).unwrap(),
            Some(())
        );
        editor.try_handle_input("/").unwrap();
        h.screen.settle().unwrap();
        for request in h.screen.take_futures() {
            tokio::task::spawn_local(request).await.unwrap().unwrap();
        }
        let rows = h.rows(80);
        let paint = maestro_theme::get_editor_theme(h.ui.theme.clone()).border_color;
        assert!(rows.len() > 3, "{rows:?}");
        assert_eq!(
            rows[2],
            paint("─").repeat(80),
            "the editor's own bottom border"
        );
        assert!(rows.last().unwrap().contains("second"), "{rows:?}");
        editor.try_handle_input("\x1b").unwrap();
        let rows = h.rows(80);
        assert_eq!(rows.len(), 3, "{rows:?}");
        assert_eq!(rows[2], h.status(80, STATUS));
    });
}

/// The home directory and the paths below it show as `~`; a sibling sharing its first
/// characters keeps its full spelling.
#[test]
fn border_status_editor_component_shortens_only_the_home_directory() {
    let test = "border_status_editor_component_shortens_only_the_home_directory";
    let component = std::env::var(COMPONENT).expect("nextest component setup");
    let environment = |_: &&str| {
        let mut variables = BTreeMap::from([
            ("HOME", "/home/al".to_owned()),
            ("USERPROFILE", "/home/al".to_owned()),
            (COMPONENT, component.clone()),
        ]);
        variables.extend(
            maestro_test_process::user_cache_home()
                .map(|path| ("XDG_CACHE_HOME", path.to_string_lossy().into_owned())),
        );
        variables
    };
    let selected = maestro_test_process::run_per_environment(test, ["home"], environment);
    if selected.unwrap().is_none() {
        return;
    }
    run(async {
        for (cwd, shown) in [
            ("/home/al", "~"),
            ("/home/al/project", "~/project"),
            ("/home/alice", "/home/alice"),
        ] {
            let h = Harness::new(cwd).await;
            h.start().await;
            let right = format!(" ctx 12%/200k · {shown} ");
            assert_eq!(h.rows(80)[2], h.status(80, &right), "{cwd}");
        }
    });
}
