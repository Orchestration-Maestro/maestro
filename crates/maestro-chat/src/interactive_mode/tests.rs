//! Native presentation witnesses through the interactive mode's own interface.
mod actions;
mod models;
#[allow(
    dead_code,
    unused_imports,
    reason = "Only executable installation is shared."
)]
#[path = "../../../maestro-app/tests/support/native.rs"]
mod native;
#[path = "../../tests/footer_support/session.rs"]
mod session;
mod transcript;
mod typed;

use crate::{InteractiveMode, InteractiveModeOptions};
// The controlled host, disposable directory and palettes are compiled once in the library tests.
use crate::components::session_selector::list::tests::session_picker::{
    manual_runtime::ManualRuntime, native_child as child, published,
    recording_terminal::RecordingTerminal,
};
use maestro_app::configuration::config::{ENV_AGENT_DIR, VERSION};
use maestro_app::contributions::LoadContributionsResult;
use maestro_app::presentation_data::footer_data_provider::NativeFooterOperations;
use maestro_app::{
    AgentSessionRuntime, ContextFile, CreateAgentSessionRuntimeFactory, DefaultEventBus,
    DiagnosticErrorInfo, ExtensionFactory, LoadExtensionsResult, LoadPromptsResult,
    LoadSkillsResult, LoadThemesResult, Local, ResourceExtensionPaths, ResourceLoader,
    SettingsManager, create_extension_runtime, load_extension_from_factory,
};
use maestro_test_process::TestProcess;
use maestro_theme::{Theme, ThemeColor, ThemeState};
use maestro_tui::{
    AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions, Cancellation, Component,
    EditorComponent, autocomplete,
};
use serde::Deserialize;
use std::{cell::RefCell, future::Future, io, path::Path, pin::Pin, rc::Rc};

/// Every recorded presentation case.
const FIXTURES: &str = include_str!("../../tests/fixtures/interactive_startup.json");

/// Runs `body` in a fresh single-test child whose agent directory is a disposable root,
/// so the constructor reads its key bindings from there rather than the developer's.
/// The child's temporary directory is that root's `tmp`, its commands are looked up in
/// the root's `bin` first, and its clipboard is a Wayland one.
fn isolated(test: &str, body: impl FnOnce(&Path)) {
    if maestro_test_process::selection().is_child() {
        let directory = std::env::var(&*ENV_AGENT_DIR).unwrap();
        body(Path::new(&directory));
        return;
    }
    let scratch = child::Scratch::new();
    let root = scratch.0.path();
    for name in ["bin", "tmp"] {
        std::fs::create_dir(root.join(name)).unwrap();
    }
    let path = format!(
        "{}:{}",
        root.join("bin").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    child::probe(
        TestProcess::current(test)
            .unwrap()
            .case("child")
            .temp_dir(root.join("tmp")),
        root,
        &[
            (ENV_AGENT_DIR.as_str(), root.to_str().unwrap()),
            ("PATH", &path),
            ("WAYLAND_DISPLAY", "maestro-test"),
        ],
        &["WSL_DISTRO_NAME", "WSLENV", "TERMUX_VERSION"],
    );
}

/// A resource loader that forwards to the default one, answering supplied themes and
/// changing the configured theme while its themes are read.
struct ForwardingLoader {
    /// The default loader every other getter forwards to.
    inner: Rc<dyn ResourceLoader<Local>>,
    /// The loaded themes.
    themes: Vec<Rc<Theme>>,
    /// The settings and theme name selected while the themes are read.
    select: RefCell<Option<(Rc<RefCell<SettingsManager>>, String)>>,
    /// The loaded extensions answered instead of the default loader's.
    extensions: Option<Rc<LoadExtensionsResult<Local>>>,
}

impl ResourceLoader<Local> for ForwardingLoader {
    fn get_extensions(&self) -> Rc<LoadExtensionsResult<Local>> {
        self.extensions
            .clone()
            .unwrap_or_else(|| self.inner.get_extensions())
    }
    fn get_skills(&self) -> Rc<LoadSkillsResult> {
        self.inner.get_skills()
    }
    fn get_prompts(&self) -> Rc<LoadPromptsResult> {
        self.inner.get_prompts()
    }
    fn get_themes(&self) -> Rc<LoadThemesResult> {
        if let Some((settings, name)) = self.select.borrow_mut().take() {
            settings.borrow_mut().set_theme(name);
        }
        Rc::new(LoadThemesResult {
            themes: self.themes.clone(),
            diagnostics: Vec::new(),
        })
    }
    fn get_contributions(&self) -> Rc<LoadContributionsResult> {
        self.inner.get_contributions()
    }
    fn get_agents_files(&self) -> Rc<[ContextFile]> {
        self.inner.get_agents_files()
    }
    fn get_system_prompt(&self) -> Option<Rc<str>> {
        self.inner.get_system_prompt()
    }
    fn get_append_system_prompt(&self) -> Rc<[String]> {
        self.inner.get_append_system_prompt()
    }
    fn extend_resources(&self, paths: ResourceExtensionPaths) -> io::Result<()> {
        self.inner.extend_resources(paths)
    }
    fn reload(&self) -> Pin<Box<dyn Future<Output = io::Result<()>> + '_>> {
        self.inner.reload()
    }
}

/// The dark palette the loader registers.
fn dark() -> Rc<Theme> {
    published::named(0, Some("dark"))
}

/// The light palette the loader registers, in other colours.
fn light() -> Rc<Theme> {
    published::named(100, Some("light"))
}

/// One mode over a recording terminal, a controlled host and the loader's themes.
struct World {
    /// The mode under test.
    mode: Rc<InteractiveMode>,
    /// The terminal the frame writer writes to.
    terminal: RecordingTerminal,
    /// The host that runs scheduled frames.
    runtime: ManualRuntime,
    /// The theme registry the mode selected from.
    themes: Rc<ThemeState>,
    /// The runtime the mode reads its session from.
    host: Rc<AgentSessionRuntime>,
}

/// Builds the inputs of one mode, letting `configure` change settings and the theme
/// selected while the loader's themes are read.
struct Setup {
    /// The mode options.
    options: InteractiveModeOptions,
    /// Changes the settings before construction.
    configure: Box<dyn FnOnce(&mut SettingsManager)>,
    /// The theme name selected inside the loader's theme getter.
    select: Option<String>,
    /// The inline extension the session loads, when supplied.
    extension: Option<ExtensionFactory<Local>>,
    /// Creates replacement sessions, when supplied; replacing otherwise fails.
    create: Option<CreateAgentSessionRuntimeFactory>,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            options: InteractiveModeOptions::default(),
            configure: Box::new(|_| {}),
            select: None,
            extension: None,
            create: None,
        }
    }
}

/// Loads `factory` as the session's only extension; its initialization must finish at once.
fn loaded(cwd: &str, factory: ExtensionFactory<Local>) -> Rc<LoadExtensionsResult<Local>> {
    let local = Rc::new(tokio::task::LocalSet::new());
    let bus = <Local as DefaultEventBus>::default_event_bus(&local);
    let runtime = create_extension_runtime::<Local>();
    let loading = std::pin::pin!(load_extension_from_factory(
        factory,
        cwd.to_owned(),
        bus.for_extension(),
        Rc::clone(&runtime),
        None,
    ));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    let std::task::Poll::Ready(owner) = std::future::Future::poll(loading, &mut context) else {
        panic!("the inline extension initializes at once");
    };
    Rc::new(LoadExtensionsResult {
        extensions: vec![owner.unwrap()],
        errors: Vec::new(),
        runtime,
    })
}

impl Setup {
    /// Constructs the mode in `directory`.
    fn build(self, directory: &Path) -> World {
        let cwd = directory.to_str().unwrap();
        let loader = Rc::new(RefCell::new(None::<Rc<ForwardingLoader>>));
        let created = Rc::clone(&loader);
        let extensions = self.extension.map(|factory| loaded(cwd, factory));
        let result = session::session(cwd, None, move |inner| {
            let themes = Rc::new(ForwardingLoader {
                inner,
                themes: vec![dark(), light()],
                select: RefCell::default(),
                extensions,
            });
            *created.borrow_mut() = Some(Rc::clone(&themes));
            themes
        });
        let settings = Rc::clone(result.session.settings_manager());
        settings.borrow_mut().set_theme("dark".to_owned());
        (self.configure)(&mut settings.borrow_mut());
        if let Some(name) = self.select {
            let loader = loader.borrow();
            *loader.as_ref().unwrap().select.borrow_mut() = Some((settings, name));
        }
        let create = self.create.unwrap_or_else(|| {
            Rc::new(|_| Box::pin(async { Err(DiagnosticErrorInfo::new(None, "unused")) }))
        });
        let host = Rc::new(AgentSessionRuntime::new(result, create));
        let terminal = RecordingTerminal::new(80, 24);
        let runtime = ManualRuntime::new();
        let themes = Rc::new(published::unpublished());
        let local = Rc::new(tokio::task::LocalSet::new());
        let mode = InteractiveMode::new(
            Rc::clone(&host),
            self.options,
            (terminal.handle(), runtime.handle()),
            Rc::clone(&themes),
            Rc::new(NativeFooterOperations::new(local)),
        )
        .unwrap();
        World {
            mode,
            terminal,
            runtime,
            themes,
            host,
        }
    }
}

/// Builds a default mode in `directory` and initializes it, settling the first frame.
fn started(directory: &Path) -> World {
    let world = Setup::default().build(directory);
    world.mode.init().unwrap();
    world.runtime.settle().unwrap();
    world
}

/// The published theme's escape for each recorded role marker.
fn roles(theme: &Theme, text: &str) -> String {
    let mut text = text.replace("<version>", VERSION);
    for (marker, role) in [
        ("@accent@", ThemeColor::Accent),
        ("@dim@", ThemeColor::Dim),
        ("@muted@", ThemeColor::Muted),
        ("@error@", ThemeColor::Error),
        ("@warning@", ThemeColor::Warning),
        ("@thinkingOff@", ThemeColor::ThinkingOff),
        ("@thinkingMinimal@", ThemeColor::ThinkingMinimal),
        ("@thinkingLow@", ThemeColor::ThinkingLow),
        ("@thinkingMedium@", ThemeColor::ThinkingMedium),
        ("@thinkingHigh@", ThemeColor::ThinkingHigh),
        ("@thinkingXhigh@", ThemeColor::ThinkingXhigh),
        ("@bashMode@", ThemeColor::BashMode),
    ] {
        text = text.replace(marker, theme.get_fg_ansi(&role).unwrap());
    }
    text
}

/// Expected rows with each role marker replaced by the published theme's escape.
fn painted(theme: &Theme, rows: &[String]) -> Vec<String> {
    rows.iter().map(|row| roles(theme, row)).collect()
}

/// One recorded case of this module's witnesses.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// The witness the case belongs to.
    test: String,
    /// The recorded operation.
    input: Input,
    /// The recorded observation.
    expected: Expected,
}

/// The recorded operations.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
pub(super) enum Input {
    /// The startup header under quiet, verbose and configured key settings.
    Header {
        /// Whether startup is quiet.
        quiet: bool,
        /// Whether the expanded help is forced.
        verbose: bool,
        /// Whether tool output was expanded when recorded.
        expanded: bool,
        /// The key binding file's contents.
        config: serde_json::Value,
        /// The render width.
        width: usize,
    },
    /// The help text's two renderings, checked by the startup module's own witness.
    Expandable {
        /// The compact text.
        collapsed: String,
        /// The expanded text.
        expanded: String,
        /// The initial state.
        initial: bool,
        /// The render width.
        width: usize,
    },
    /// One labelled transcript notice.
    Notice {
        /// The helper called.
        kind: NoticeKind,
        /// The caller's text.
        text: String,
        /// The render width.
        width: usize,
    },
    /// The editor cleared through the mode.
    Clear,
}

/// The two notice helpers.
#[derive(Deserialize)]
pub(super) enum NoticeKind {
    /// The error helper.
    #[serde(rename = "showError")]
    Error,
    /// The warning helper.
    #[serde(rename = "showWarning")]
    Warning,
}

/// The recorded observations.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub(super) enum Expected {
    /// Header rows and the header's child count.
    Header {
        /// The header rows.
        before: Vec<String>,
        /// The header's children.
        children: usize,
    },
    /// The help text's rows before and after switching.
    Expandable {
        /// Rows before switching.
        before: Vec<String>,
        /// Rows after switching.
        after: Vec<String>,
    },
    /// Notice rows and the frame requests.
    Notice {
        /// The transcript rows.
        lines: Vec<String>,
        /// Frames requested.
        renders: usize,
    },
    /// The editor text and the frame requests.
    Clear {
        /// The editor text.
        text: String,
        /// Frames requested.
        renders: usize,
    },
}

/// The recorded cases of `test`.
pub(super) fn cases(test: &str) -> Vec<(Input, Expected)> {
    let all: Vec<Case> = serde_json::from_str(FIXTURES).unwrap();
    let cases: Vec<_> = all
        .into_iter()
        .filter(|case| case.test == test)
        .map(|case| (case.input, case.expected))
        .collect();
    assert!(!cases.is_empty());
    cases
}

#[test]
fn startup_header_keeps_quiet_verbose_and_configured_hints_distinct() {
    isolated(
        "startup_header_keeps_quiet_verbose_and_configured_hints_distinct",
        |directory| {
            for (input, expected) in
                cases("startup_header_keeps_quiet_verbose_and_configured_hints_distinct")
            {
                let (
                    Input::Header {
                        quiet,
                        verbose,
                        expanded,
                        config,
                        width,
                    },
                    Expected::Header { before, children },
                ) = (input, expected)
                else {
                    panic!("a header case");
                };
                assert!(!expanded, "tool output starts collapsed");
                std::fs::write(directory.join("keybindings.json"), config.to_string()).unwrap();
                let world = Setup {
                    options: InteractiveModeOptions { verbose },
                    configure: Box::new(move |settings| settings.set_quiet_startup(quiet)),
                    ..Setup::default()
                }
                .build(directory);
                world.mode.init().unwrap();
                let theme = world.themes.theme().get().unwrap();
                let expected = painted(&theme, &before);
                assert_eq!(
                    world.mode.header_container.children().borrow().len(),
                    children
                );
                assert_eq!(world.mode.header_container.render(width), expected);
                assert_eq!(world.mode.ui.render(width)[..expected.len()], expected);
            }
        },
    );
}

#[test]
fn startup_notices_keep_labels_styles_padding_and_redraw() {
    isolated(
        "startup_notices_keep_labels_styles_padding_and_redraw",
        |directory| {
            for (input, expected) in cases("startup_notices_keep_labels_styles_padding_and_redraw")
            {
                let (Input::Notice { kind, text, width }, Expected::Notice { lines, renders }) =
                    (input, expected)
                else {
                    panic!("a notice case");
                };
                let world = started(directory);
                assert_eq!(world.runtime.pending(), 0);
                match kind {
                    NoticeKind::Error => world.mode.show_error(&text),
                    NoticeKind::Warning => world.mode.show_warning(&text),
                }
                .unwrap();
                assert_eq!(world.runtime.pending(), renders);
                let theme = world.themes.theme().get().unwrap();
                let expected = painted(&theme, &lines);
                assert_eq!(world.mode.chat_container.render(width), expected);
                let header = world.mode.header_container.render(width).len();
                let frame = world.mode.ui.render(width);
                assert_eq!(frame[header..header + expected.len()], expected);
            }
        },
    );
}

#[test]
fn clear_editor_changes_text_and_requests_a_frame() {
    isolated(
        "clear_editor_changes_text_and_requests_a_frame",
        |directory| {
            let [(Input::Clear, Expected::Clear { text, renders })] =
                <[_; 1]>::try_from(cases("clear_editor_changes_text_and_requests_a_frame"))
                    .ok()
                    .unwrap()
            else {
                panic!("one clear case");
            };
            let world = started(directory);
            world.mode.editor.add_to_history("remembered");
            world.mode.editor.set_text("draft");
            world.runtime.settle().unwrap();
            world.mode.clear_editor();
            assert_eq!(world.mode.editor.get_text(), text);
            assert_eq!(world.runtime.pending(), renders);
            world.terminal.send_input("\x1b[A");
            assert_eq!(world.mode.editor.get_text(), "remembered");
        },
    );
}

/// A provider answering `count` numbered candidates.
struct Numbered(usize);

impl AutocompleteProvider for Numbered {
    type Signal = Cancellation;
    fn get_suggestions<'a>(
        &'a self,
        _lines: &'a [String],
        _cursor: autocomplete::CursorPosition,
        _options: autocomplete::CompletionOptions<'a, Self::Signal>,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<Option<AutocompleteSuggestions>, autocomplete::CompletionError>,
                > + 'a,
        >,
    > {
        let items = (0..self.0)
            .map(|index| AutocompleteItem {
                value: format!("item{index}"),
                label: format!("item{index}"),
                description: None,
            })
            .collect();
        Box::pin(async {
            Ok(Some(AutocompleteSuggestions {
                items,
                prefix: String::new(),
            }))
        })
    }
    fn apply_completion(
        &self,
        _lines: &[String],
        _cursor: autocomplete::CursorPosition,
        item: &AutocompleteItem,
        _prefix: &str,
    ) -> autocomplete::CompletionResult {
        autocomplete::CompletionResult {
            lines: vec![item.value.clone()],
            cursor_line: 0,
            cursor_col: item.value.len(),
        }
    }
}

/// The candidate rows the mode's editor shows with `maximum` configured.
fn visible_candidates(directory: &Path, maximum: f64) -> usize {
    let world = Setup {
        configure: Box::new(move |settings| settings.set_autocomplete_max_visible(maximum)),
        ..Setup::default()
    }
    .build(directory);
    world.mode.init().unwrap();
    world
        .mode
        .editor
        .set_autocomplete_provider(Rc::new(Numbered(30)));
    world.terminal.send_input("\t");
    assert!(world.runtime.drain_futures().is_empty());
    let rows = world.mode.editor.render(80);
    rows.iter().filter(|row| row.contains("item")).count()
}

/// The leading spaces of the editor's text row with `padding` configured.
fn editor_indent(directory: &Path, padding: f64) -> usize {
    let world = Setup {
        configure: Box::new(move |settings| settings.set_editor_padding_x(padding)),
        ..Setup::default()
    }
    .build(directory);
    world.mode.init().unwrap();
    world.mode.editor.set_text("typed");
    let rows = world.mode.editor.render(40);
    let row = rows.iter().find(|row| row.contains("typed")).unwrap();
    row.len() - row.trim_start_matches(' ').len()
}

#[test]
fn startup_constructor_forwards_each_display_setting() {
    isolated(
        "startup_constructor_forwards_each_display_setting",
        |directory| {
            for (cursor, shrink) in [(true, false), (false, true)] {
                let world = Setup {
                    configure: Box::new(move |settings| {
                        settings.set_show_hardware_cursor(cursor);
                        settings.set_clear_on_shrink(shrink);
                    }),
                    ..Setup::default()
                }
                .build(directory);
                assert_eq!(world.mode.ui.get_show_hardware_cursor(), cursor);
                assert_eq!(world.mode.ui.get_clear_on_shrink(), shrink);
                world.mode.init().unwrap();
                world.runtime.settle().unwrap();
                world.mode.show_error("tall").unwrap();
                world.runtime.settle().unwrap();
                let redraws = world.mode.ui.full_redraws();
                world.mode.chat_container.clear();
                world.mode.ui.request_render(false);
                world.runtime.settle().unwrap();
                assert_eq!(world.mode.ui.full_redraws() > redraws, shrink);
            }
            let indents: Vec<_> = [0.0, 1.0, 2.0]
                .into_iter()
                .map(|padding| editor_indent(directory, padding))
                .collect();
            assert_eq!(indents, [0, 1, 2]);
            let visible: Vec<_> = [3.0, 5.0, 20.0]
                .into_iter()
                .map(|maximum| visible_candidates(directory, maximum))
                .collect();
            assert_eq!(visible, [3, 5, 20]);
            for enabled in [true, false] {
                let world = Setup {
                    configure: Box::new(move |settings| settings.set_compaction_enabled(enabled)),
                    ..Setup::default()
                }
                .build(directory);
                world.mode.init().unwrap();
                let footer = world.mode.footer.render(120).join("\n");
                assert_eq!(footer.contains("(auto)"), enabled);
            }
        },
    );
}

#[test]
fn startup_constructor_registers_themes_before_selection() {
    isolated(
        "startup_constructor_registers_themes_before_selection",
        |directory| {
            let world = Setup::default().build(directory);
            let published = world.themes.theme().get().unwrap();
            assert!(Rc::ptr_eq(&published, &world.themes.theme().get().unwrap()));
            assert_eq!(published.name(), Some("dark"));
            assert_eq!(
                published.get_fg_ansi(&ThemeColor::Accent).unwrap(),
                dark().get_fg_ansi(&ThemeColor::Accent).unwrap()
            );
            for (initial, next) in [("dark", "light"), ("light", "dark")] {
                let world = Setup {
                    configure: Box::new(move |settings| settings.set_theme(initial.to_owned())),
                    select: Some(next.to_owned()),
                    ..Setup::default()
                }
                .build(directory);
                let published = world.themes.theme().get().unwrap();
                assert_eq!(published.name(), Some(next));
            }
        },
    );
}

#[test]
fn startup_layout_focus_and_start_happen_once() {
    isolated("startup_layout_focus_and_start_happen_once", |directory| {
        let world = started(directory);
        let mode = &world.mode;
        let expected: Vec<Rc<dyn Component>> = vec![
            mode.header_container.clone(),
            mode.chat_container.clone(),
            mode.pending_messages_container.clone(),
            mode.status_container.clone(),
            mode.widget_container_above.clone(),
            mode.editor_container.clone(),
            mode.widget_container_below.clone(),
            mode.footer.clone(),
        ];
        let same = |children: &[Rc<dyn Component>]| {
            children.len() == expected.len()
                && children.iter().zip(&expected).all(|(child, expected)| {
                    std::ptr::addr_eq(Rc::as_ptr(child), Rc::as_ptr(expected))
                })
        };
        assert!(same(&mode.ui.children().borrow()));
        assert!(mode.widget_container_above.children().borrow().is_empty());
        assert!(mode.widget_container_below.children().borrow().is_empty());
        world.terminal.send_input("x");
        assert_eq!(mode.editor.get_text(), "x");
        mode.init().unwrap();
        assert!(same(&mode.ui.children().borrow()));
        assert_eq!(world.terminal.events(), ["start"]);
        assert_eq!(mode.header_container.children().borrow().len(), 3);
    });
}

#[test]
fn startup_failed_terminal_start_does_not_mark_initialized() {
    isolated(
        "startup_failed_terminal_start_does_not_mark_initialized",
        |directory| {
            let world = Setup::default().build(directory);
            world
                .terminal
                .set_hide_error(Some(io::ErrorKind::BrokenPipe));
            let error = world.mode.init().unwrap_err();
            assert_eq!(
                error.downcast_ref::<io::Error>().unwrap().kind(),
                io::ErrorKind::BrokenPipe
            );
            assert!(!world.mode.is_initialized.get());
        },
    );
}

#[test]
fn startup_theme_and_branch_notifications_request_fresh_frames() {
    isolated(
        "startup_theme_and_branch_notifications_request_fresh_frames",
        |directory| {
            let world = started(directory);
            let header = world.mode.header_container.render(80);
            let footer = world.mode.footer.render(80);
            world.themes.set_theme_instance(light()).unwrap();
            assert_eq!(world.runtime.pending(), 1);
            world.terminal.clear_writes();
            world.runtime.settle().unwrap();
            assert!(!world.terminal.writes().is_empty());
            assert_eq!(world.mode.header_container.render(80), header);
            assert_ne!(world.mode.footer.render(80), footer);
            let other = directory.join("elsewhere");
            std::fs::create_dir(&other).unwrap();
            world
                .mode
                .footer_data_provider
                .set_cwd(other.to_str().unwrap().to_owned());
            assert_eq!(world.runtime.pending(), 1);
        },
    );
}

#[test]
fn startup_callback_registration_does_not_retain_the_mode() {
    isolated(
        "startup_callback_registration_does_not_retain_the_mode",
        |directory| {
            let World {
                mode,
                terminal,
                runtime,
                themes,
                host,
            } = started(directory);
            let provider = mode.footer_data_provider.clone();
            let weak = Rc::downgrade(&mode);
            drop(mode);
            assert!(weak.upgrade().is_none());
            themes.set_theme_instance(light()).unwrap();
            provider.set_cwd(directory.join("gone").to_str().unwrap().to_owned());
            assert_eq!(runtime.pending(), 0);
            drop((terminal, host));
        },
    );
}
