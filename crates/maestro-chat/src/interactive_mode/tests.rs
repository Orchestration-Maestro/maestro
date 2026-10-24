//! Native presentation witnesses through the interactive mode's own interface.
mod actions;
mod bash;
mod events;
mod extension_ui;
#[allow(
    dead_code,
    unused_imports,
    reason = "Only executable installation is shared."
)]
#[path = "../../../maestro-app/tests/support/native.rs"]
mod native;
mod notices;
mod prompts;
mod queues;
#[path = "../../tests/footer_support/session.rs"]
mod session;
mod transcript;
mod transfer;
mod typed;

use crate::{InteractiveMode, InteractiveModeOptions};
// The controlled host, disposable directory and palettes are compiled once in the library tests.
use crate::components::session_selector::list::tests::session_picker::{
    manual_runtime::ManualRuntime, native_child as child, published,
    recording_terminal::RecordingTerminal,
};
use maestro_app::configuration::config::{APP_NAME, ENV_AGENT_DIR, VERSION};
use maestro_app::contributions::LoadContributionsResult;
use maestro_app::presentation_data::footer_data_provider::{
    FooterOperations, NativeFooterOperations,
};
use maestro_app::{
    AgentSessionRuntime, ContextFile, DefaultEventBus, Extension, ExtensionCommandContext,
    ExtensionFactory, ExtensionLoadError, LoadExtensionsResult, LoadPromptsResult,
    LoadSkillsResult, LoadThemesResult, Local, PromptTemplate, RegisteredCommand,
    ResourceDiagnostic, ResourceExtensionPaths, ResourceLoader, SettingsManager, Skill, SourceInfo,
    SourceOrigin, SourceScope, create_extension_runtime, load_extension_from_factory,
};
use maestro_test_process::TestProcess;
use maestro_theme::{Theme, ThemeColor, ThemeState};
use maestro_tui::{
    AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions, Cancellation, Component,
    EditorComponent, autocomplete,
};
use serde::{Deserialize, de::DeserializeOwned};
use std::{
    cell::RefCell,
    collections::VecDeque,
    future::Future,
    io,
    path::Path,
    pin::Pin,
    rc::Rc,
    sync::{Arc, LazyLock, RwLock},
};

/// The package-directory environment key the installation reads.
pub(super) static ENV_PACKAGE_DIR: LazyLock<String> =
    LazyLock::new(|| format!("{}_PACKAGE_DIR", APP_NAME.to_uppercase()));

/// Every recorded presentation case.
pub(super) const FIXTURES: &str = include_str!("../../tests/fixtures/interactive_startup.json");

/// Runs `body` in a fresh single-test child whose working directory is a disposable root
/// and whose package directory is `package` inside it.
pub(super) fn isolated_package(test: &str, body: impl FnOnce(&Path, &Path)) {
    if maestro_test_process::selection().is_child() {
        let directory = std::env::var(&*ENV_AGENT_DIR).unwrap();
        let package = std::env::var(&*ENV_PACKAGE_DIR).unwrap();
        body(Path::new(&directory), Path::new(&package));
        return;
    }
    let scratch = child::Scratch::new();
    let directory = scratch.0.path();
    let package = directory.join("package");
    std::fs::create_dir(&package).unwrap();
    child::probe(
        TestProcess::current(test).unwrap().case("child"),
        directory,
        &[
            (ENV_AGENT_DIR.as_str(), directory.to_str().unwrap()),
            (ENV_PACKAGE_DIR.as_str(), package.to_str().unwrap()),
        ],
        &[],
    );
}

/// Runs `body` in a fresh single-test child whose agent directory is a disposable root,
/// so the constructor reads its key bindings from there rather than the developer's.
fn isolated(test: &str, body: impl FnOnce(&Path)) {
    isolated_case(test, "child", &[], body);
}

/// Runs `body` as `case` of `test` in a fresh single-test child over a disposable agent
/// directory, with no inherited environment apart from the command path and `envs`, so
/// no developer credential reaches it. The child's temporary directory is that root's
/// `tmp`, its commands are looked up in the root's `bin` first, and its clipboard is a
/// Wayland one. The file-size signal is ignored, so a write over a lowered limit fails
/// instead of ending the child.
pub(super) fn isolated_case(
    test: &str,
    case: &str,
    envs: &[(&str, &str)],
    body: impl FnOnce(&Path),
) {
    let selection = maestro_test_process::selection();
    if selection.is_child() {
        if selection.runs_case(case) {
            let directory = std::env::var(&*ENV_AGENT_DIR).unwrap();
            body(Path::new(&directory));
        }
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
    let mut all = vec![
        (ENV_AGENT_DIR.as_str(), root.to_str().unwrap()),
        ("PATH", &path),
        ("WAYLAND_DISPLAY", "maestro-test"),
    ];
    all.extend_from_slice(envs);
    child::probe(
        TestProcess::current(test)
            .unwrap()
            .env_clear()
            .case(case)
            .shell_setup("trap '' XFSZ")
            .temp_dir(root.join("tmp")),
        root,
        &all,
        &[],
    );
}

/// A resource loader that forwards to the default one, answering supplied themes, skills,
/// successive prompt templates, context files and loaded extensions, and changing the
/// configured theme while its themes are read.
pub(super) struct ForwardingLoader {
    /// The default loader every other getter forwards to.
    inner: Rc<dyn ResourceLoader<Local>>,
    /// The loaded themes and their diagnostics.
    themes: Rc<LoadThemesResult>,
    /// The settings and theme name selected while the themes are read.
    select: RefCell<Option<(Rc<RefCell<SettingsManager>>, String)>>,
    /// The discovered context files, replaceable after construction.
    pub(super) context: RefCell<Rc<[ContextFile]>>,
    /// The loaded skills.
    skills: Rc<LoadSkillsResult>,
    /// The prompt templates of each read; the last answers every later read.
    prompts: RefCell<VecDeque<Rc<LoadPromptsResult>>>,
    /// The loaded extensions, the default loader's when absent.
    extensions: Option<Rc<LoadExtensionsResult<Local>>>,
    /// The getters read, in order.
    pub(super) calls: RefCell<Vec<&'static str>>,
}

/// The diagnostics a loader reports beside its skills, prompt templates and themes.
#[derive(Default)]
pub(super) struct LoadedDiagnostics {
    /// The skill diagnostics.
    pub(super) skills: Vec<ResourceDiagnostic>,
    /// The prompt template diagnostics of the first read.
    pub(super) prompts: Vec<ResourceDiagnostic>,
    /// The theme diagnostics.
    pub(super) themes: Vec<ResourceDiagnostic>,
}

/// The resources a loader answers.
struct Resources {
    /// The context files.
    context: Vec<ContextFile>,
    /// The skills.
    skills: Vec<Skill>,
    /// The prompt templates of each read; the last answers every later read.
    prompts: Vec<Vec<PromptTemplate>>,
    /// The loaded themes listed before the dark and light palettes.
    themes: Vec<Rc<Theme>>,
    /// The loaded extensions by path and provenance.
    extensions: Vec<(String, SourceInfo)>,
    /// The paths that failed to load.
    errors: Vec<ExtensionLoadError>,
    /// The commands registered, each by its own extension, by name and path.
    commands: Vec<(String, String)>,
    /// The diagnostics beside the skills, prompt templates and themes.
    diagnostics: LoadedDiagnostics,
}

impl ForwardingLoader {
    /// A loader over `inner` answering `resources`, then the dark and light palettes, and
    /// the supplied `extensions` before the listed ones.
    fn new(
        inner: Rc<dyn ResourceLoader<Local>>,
        resources: Resources,
        extensions: Option<Rc<LoadExtensionsResult<Local>>>,
    ) -> Self {
        let Resources {
            context,
            skills,
            mut prompts,
            mut themes,
            extensions: paths,
            errors,
            commands,
            diagnostics,
        } = resources;
        if prompts.is_empty() && !diagnostics.prompts.is_empty() {
            prompts.push(Vec::new());
        }
        let mut prompt_diagnostics = Some(diagnostics.prompts);
        let prompts = prompts.into_iter().map(|prompts| {
            Rc::new(LoadPromptsResult {
                prompts,
                diagnostics: prompt_diagnostics.take().unwrap_or_default(),
            })
        });
        themes.extend([dark(), light()]);
        let loaded = !(paths.is_empty() && errors.is_empty() && commands.is_empty());
        Self {
            inner,
            themes: Rc::new(LoadThemesResult {
                themes,
                diagnostics: diagnostics.themes,
            }),
            select: RefCell::default(),
            context: RefCell::new(context.into()),
            skills: Rc::new(LoadSkillsResult {
                skills,
                diagnostics: diagnostics.skills,
            }),
            prompts: RefCell::new(prompts.collect()),
            extensions: extensions
                .or_else(|| loaded.then(|| Rc::new(load(paths, errors, &commands)))),
            calls: RefCell::default(),
        }
    }
}

/// The command callback a registration stores; the witnesses never invoke it.
type CommandHandler = Rc<
    dyn Fn(
        String,
        ExtensionCommandContext,
    )
        -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>>>>,
>;

/// One loaded extension at `path` with its provenance, registering nothing.
fn extension(path: String, source_info: SourceInfo) -> Rc<RwLock<Extension<Local>>> {
    let source_info = Arc::new(RwLock::new(source_info));
    Rc::new(RwLock::new(Extension::new(path.clone(), path, source_info)))
}

/// One local project extension at `path` registering the command `name`, as the
/// extension registration interface stores it.
fn commanding(name: &str, path: &str) -> Rc<RwLock<Extension<Local>>> {
    let source_info = SourceInfo {
        path: path.to_owned(),
        source: "local".to_owned(),
        scope: SourceScope::Project,
        origin: SourceOrigin::TopLevel,
        base_dir: None,
    };
    let owner = extension(path.to_owned(), source_info);
    let handler: CommandHandler = Rc::new(|_, _| Box::pin(async { Ok(()) }));
    let command = {
        let owner = owner.read().unwrap();
        RegisteredCommand {
            name: name.to_owned(),
            source_info: Arc::clone(&owner.source_info),
            description: None,
            get_argument_completions: None,
            handler,
        }
    };
    let command = Rc::new(RwLock::new(command));
    owner
        .write()
        .unwrap()
        .commands
        .insert(name.to_owned(), command);
    owner
}

/// Extensions loaded from `paths` with their provenance, then one extension per
/// command, and the paths that failed.
fn load(
    paths: Vec<(String, SourceInfo)>,
    errors: Vec<ExtensionLoadError>,
    commands: &[(String, String)],
) -> LoadExtensionsResult<Local> {
    let extensions = paths
        .into_iter()
        .map(|(path, source_info)| extension(path, source_info))
        .chain(commands.iter().map(|(name, path)| commanding(name, path)));
    LoadExtensionsResult {
        extensions: extensions.collect(),
        errors,
        runtime: create_extension_runtime::<Local>(),
    }
}

impl ResourceLoader<Local> for ForwardingLoader {
    fn get_extensions(&self) -> Rc<LoadExtensionsResult<Local>> {
        self.calls.borrow_mut().push("extensions");
        self.extensions
            .clone()
            .unwrap_or_else(|| self.inner.get_extensions())
    }
    fn get_skills(&self) -> Rc<LoadSkillsResult> {
        self.calls.borrow_mut().push("skills");
        Rc::clone(&self.skills)
    }
    fn get_prompts(&self) -> Rc<LoadPromptsResult> {
        self.calls.borrow_mut().push("prompts");
        let mut prompts = self.prompts.borrow_mut();
        if prompts.len() > 1 {
            prompts.pop_front().unwrap()
        } else {
            prompts
                .front()
                .cloned()
                .unwrap_or_else(|| self.inner.get_prompts())
        }
    }
    fn get_themes(&self) -> Rc<LoadThemesResult> {
        self.calls.borrow_mut().push("themes");
        if let Some((settings, name)) = self.select.borrow_mut().take() {
            settings.borrow_mut().set_theme(name);
        }
        Rc::clone(&self.themes)
    }
    fn get_contributions(&self) -> Rc<LoadContributionsResult> {
        self.inner.get_contributions()
    }
    fn get_agents_files(&self) -> Rc<[ContextFile]> {
        self.calls.borrow_mut().push("agents");
        Rc::clone(&self.context.borrow())
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
pub(super) fn dark() -> Rc<Theme> {
    published::named(0, Some("dark"))
}

/// The light palette the loader registers, in other colours.
pub(super) fn light() -> Rc<Theme> {
    published::named(100, Some("light"))
}

/// One mode over a recording terminal, a controlled host and the loader's themes.
pub(super) struct World {
    /// The mode under test.
    pub(super) mode: Rc<InteractiveMode>,
    /// The terminal the frame writer writes to.
    pub(super) terminal: RecordingTerminal,
    /// The host that runs scheduled frames.
    runtime: ManualRuntime,
    /// The theme registry the mode selected from.
    pub(super) themes: Rc<ThemeState>,
    /// The runtime the mode reads its session from.
    host: Rc<AgentSessionRuntime>,
    /// The loader the session reads its resources from.
    pub(super) loader: Rc<ForwardingLoader>,
}

/// Builds the inputs of one mode, letting `configure` change settings and the theme
/// selected while the loader's themes are read.
pub(super) struct Setup {
    /// The mode options.
    pub(super) options: InteractiveModeOptions,
    /// Changes the settings before construction.
    pub(super) configure: Box<dyn FnOnce(&mut SettingsManager)>,
    /// The theme name selected inside the loader's theme getter.
    pub(super) select: Option<String>,
    /// The session's working directory, the disposable directory when absent.
    pub(super) cwd: Option<String>,
    /// The context files the loader answers.
    pub(super) context: Vec<ContextFile>,
    /// The skills the loader answers.
    pub(super) skills: Vec<Skill>,
    /// The prompt templates of each loader read; the last answers every later read.
    pub(super) prompts: Vec<Vec<PromptTemplate>>,
    /// The inline extension the session loads, when supplied.
    pub(super) extension: Option<ExtensionFactory<Local>>,
    /// Starts each replacement with fresh services instead of reusing the current ones.
    pub(super) replacing: bool,
    /// Creates the session history under a file in this directory when supplied.
    pub(super) session_history: Option<std::path::PathBuf>,
    /// The directories the theme registry reads, when it records themes on disk.
    pub(super) directories: Option<maestro_theme::ThemeDirectories>,
    /// The loaded themes the loader answers before the dark and light palettes.
    pub(super) themes: Vec<Rc<Theme>>,
    /// The loaded extensions by path and provenance; the default loader's when these,
    /// the failed paths and the commands are all empty.
    pub(super) extensions: Vec<(String, SourceInfo)>,
    /// The paths that failed to load.
    pub(super) errors: Vec<ExtensionLoadError>,
    /// The commands registered, each by its own extension, by name and path.
    pub(super) commands: Vec<(String, String)>,
    /// The diagnostics beside the skills, prompt templates and themes.
    pub(super) diagnostics: LoadedDiagnostics,
    /// The watch, timer and Git effects the mode uses; the native ones when absent.
    pub(super) watcher: Option<Rc<dyn FooterOperations>>,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            options: InteractiveModeOptions::default(),
            configure: Box::new(|_| {}),
            select: None,
            cwd: None,
            context: Vec::new(),
            skills: Vec::new(),
            prompts: Vec::new(),
            directories: None,
            extension: None,
            replacing: false,
            session_history: None,
            themes: Vec::new(),
            extensions: Vec::new(),
            errors: Vec::new(),
            commands: Vec::new(),
            diagnostics: LoadedDiagnostics::default(),
            watcher: None,
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
    pub(super) fn build(self, directory: &Path) -> World {
        let cwd = self
            .cwd
            .unwrap_or_else(|| directory.to_str().unwrap().to_owned());
        let loader = Rc::new(RefCell::new(None::<Rc<ForwardingLoader>>));
        let created = Rc::clone(&loader);
        let resources = Resources {
            context: self.context,
            skills: self.skills,
            prompts: self.prompts,
            themes: self.themes,
            extensions: self.extensions,
            errors: self.errors,
            commands: self.commands,
            diagnostics: self.diagnostics,
        };
        let extensions = self.extension.map(|factory| loaded(&cwd, factory));
        let history = self
            .session_history
            .as_ref()
            .map(|d| d.to_str().unwrap().to_owned());
        let result = session::session_over(
            &cwd,
            None,
            move |inner| {
                let loader = Rc::new(ForwardingLoader::new(inner, resources, extensions));
                *created.borrow_mut() = Some(Rc::clone(&loader));
                loader
            },
            history.as_deref(),
        );
        let settings = Rc::clone(result.session.settings_manager());
        prepare_settings(settings, self.configure, self.select, &loader);
        let loader = loader.borrow().clone().unwrap();
        let services = Rc::clone(&result.services);
        let host = Rc::new(AgentSessionRuntime::new(
            result,
            create(services, self.replacing),
        ));
        let terminal = RecordingTerminal::new(80, 24);
        let runtime = ManualRuntime::new();
        let themes = registry(self.directories);
        let local = Rc::new(tokio::task::LocalSet::new());
        let mode = InteractiveMode::new(
            Rc::clone(&host),
            self.options,
            (terminal.handle(), runtime.handle()),
            Rc::clone(&themes),
            self.watcher
                .unwrap_or_else(|| Rc::new(NativeFooterOperations::new(local))),
        )
        .unwrap();
        World {
            mode,
            terminal,
            runtime,
            themes,
            host,
            loader,
        }
    }
}

/// Applies the settings the caller changes and the theme name read while it answers.
fn prepare_settings(
    settings: Rc<RefCell<SettingsManager>>,
    configure: Box<dyn FnOnce(&mut SettingsManager)>,
    select: Option<String>,
    loader: &Rc<RefCell<Option<Rc<ForwardingLoader>>>>,
) {
    settings.borrow_mut().set_theme("dark".to_owned());
    (configure)(&mut settings.borrow_mut());
    if let Some(name) = select {
        *loader.borrow().as_ref().unwrap().select.borrow_mut() = Some((settings, name));
    }
}

/// The theme registry the mode selects from, unpublished without supplied directories.
fn registry(directories: Option<maestro_theme::ThemeDirectories>) -> Rc<ThemeState> {
    Rc::new(match directories {
        Some(directories) => ThemeState::new(directories),
        None => published::unpublished(),
    })
}

/// The replacement factory, reusing `services` unless fresh services are selected.
fn create(
    services: Rc<maestro_app::AgentSessionServices<Local>>,
    fresh: bool,
) -> maestro_app::CreateAgentSessionRuntimeFactory {
    Rc::new(move |options| {
        let replacement = if fresh {
            session::session(&options.cwd, None, |inner| inner)
        } else {
            session::over(&options.cwd, &services)
        };
        Box::pin(async { Ok(replacement) })
    })
}

/// Builds a default mode in `directory` and initializes it, settling the first frame.
fn started(directory: &Path) -> World {
    let world = Setup::default().build(directory);
    world.mode.init().unwrap();
    world.runtime.settle().unwrap();
    world
}

/// The published theme's escape for each recorded role marker.
pub(super) fn roles(theme: &Theme, text: &str) -> String {
    let mut text = text.replace("<version>", VERSION);
    for (marker, role) in [
        ("@border@", ThemeColor::Border),
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
        ("@mdHeading@", ThemeColor::MdHeading),
        ("@mdLink@", ThemeColor::MdLink),
        ("@success@", ThemeColor::Success),
    ] {
        text = text.replace(marker, theme.get_fg_ansi(&role).unwrap());
    }
    text
}

/// Expected rows with each role marker replaced by the published theme's escape.
pub(super) fn painted(theme: &Theme, rows: &[String]) -> Vec<String> {
    rows.iter().map(|row| roles(theme, row)).collect()
}

/// One recorded case, read before its witness gives the input and observation their types.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Case {
    /// The witness the case belongs to.
    test: String,
    /// The recorded operation.
    input: serde_json::Value,
    /// The recorded observation.
    expected: serde_json::Value,
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

/// The recorded cases of `test` in `fixtures`, typed by that witness.
pub(super) fn cases<I: DeserializeOwned, E: DeserializeOwned>(
    fixtures: &str,
    test: &str,
) -> Vec<(I, E)> {
    let all: Vec<Case> = serde_json::from_str(fixtures).unwrap();
    let cases: Vec<_> = all
        .into_iter()
        .filter(|case| case.test == test)
        .map(|case| {
            (
                serde_json::from_value(case.input).unwrap(),
                serde_json::from_value(case.expected).unwrap(),
            )
        })
        .collect();
    assert!(!cases.is_empty());
    cases
}

#[test]
fn startup_header_keeps_quiet_verbose_and_configured_hints_distinct() {
    isolated(
        "startup_header_keeps_quiet_verbose_and_configured_hints_distinct",
        |directory| {
            for (input, expected) in cases::<Input, Expected>(
                FIXTURES,
                "startup_header_keeps_quiet_verbose_and_configured_hints_distinct",
            ) {
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
                    options: InteractiveModeOptions {
                        verbose,
                        ..InteractiveModeOptions::default()
                    },
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
            for (input, expected) in cases::<Input, Expected>(
                FIXTURES,
                "startup_notices_keep_labels_styles_padding_and_redraw",
            ) {
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
                <[_; 1]>::try_from(cases::<Input, Expected>(
                    FIXTURES,
                    "clear_editor_changes_text_and_requests_a_frame",
                ))
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
                loader,
            } = started(directory);
            let provider = mode.footer_data_provider.clone();
            let weak = Rc::downgrade(&mode);
            drop(mode);
            assert!(weak.upgrade().is_none());
            themes.set_theme_instance(light()).unwrap();
            provider.set_cwd(directory.join("gone").to_str().unwrap().to_owned());
            assert_eq!(runtime.pending(), 0);
            drop((terminal, host, loader));
        },
    );
}
