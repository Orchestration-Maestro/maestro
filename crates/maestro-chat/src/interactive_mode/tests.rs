//! Native presentation witnesses through the interactive mode's own interface.
mod actions;
mod authentication;
mod autocomplete;
mod bash;
mod bindings;
mod commands;
mod debug;
mod events;
mod extension_custom;
mod extension_editors;
mod extension_replacements;
mod extension_reset;
mod extension_ui;
mod extension_widgets;
mod extensions;
mod hotkeys;
mod info;
mod lifecycle;
mod models;
#[allow(
    dead_code,
    unused_imports,
    reason = "Only executable installation is shared."
)]
#[path = "../../../maestro-app/tests/support/native.rs"]
mod native;
mod navigation;
mod notices;
mod progress;
mod prompts;
mod queues;
mod reload;
#[path = "../../tests/footer_support/session.rs"]
mod session;
mod settings;
mod share;
mod suspend;
mod thinking;
mod transcript;
mod transfer;
mod typed;

use super::hotkeys::tests::platform_keys;
use crate::components::thinking_selector::thinking_text;
use crate::interactive_mode::bash::component_address;
use crate::{BashExecutionComponent, InteractiveMode, InteractiveModeOptions};
// The controlled host, disposable directory and palettes are compiled once in the library tests.
use crate::components::session_selector::list::tests::session_picker::{
    manual_runtime::ManualRuntime, native_child as child, published,
    recording_terminal::RecordingTerminal,
};
use maestro_app::agent_session::PromptOptions;
use maestro_app::configuration::config::{APP_NAME, ENV_AGENT_DIR, VERSION};
use maestro_app::contributions::LoadContributionsResult;
use maestro_app::presentation_data::footer_data_provider::{
    FooterFileStats, FooterOperations, NativeFooterOperations,
};
use maestro_app::{
    AgentSessionRuntime, AuthStorage, ContextFile, CreateAgentSessionRuntimeFactory,
    DefaultEventBus, Extension, ExtensionCommandContext, ExtensionFactory, ExtensionLoadError,
    LoadExtensionsResult, LoadPromptsResult, LoadSkillsResult, LoadThemesResult, Local,
    PromptTemplate, RegisteredCommand, ResourceDiagnostic, ResourceExtensionPaths, ResourceLoader,
    SettingsManager, Skill, SourceInfo, SourceOrigin, SourceScope, create_extension_runtime,
    load_extension_from_factory, strip_ansi,
};
use maestro_app::{AssistantMessage, AssistantMessageEventStream, StopReason, Usage};
use maestro_test_process::TestProcess;
use maestro_theme::{
    Theme, ThemeColor, ThemeReloadTimer, ThemeState, ThemeWatchOperations, ThemeWatcher,
};
use maestro_tui::{
    AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions, Cancellation, Component,
    EditorComponent, Text, autocomplete as tui_autocomplete,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    error::Error,
    future::Future,
    io,
    path::Path,
    pin::Pin,
    rc::Rc,
    sync::{Arc, LazyLock, RwLock},
    time::Duration,
};

/// The package-directory environment key the installation reads.
pub(super) static ENV_PACKAGE_DIR: LazyLock<String> =
    LazyLock::new(|| format!("{}_PACKAGE_DIR", APP_NAME.to_uppercase()));

/// Every recorded presentation case.
pub(super) const FIXTURES: &str = include_str!("../../tests/fixtures/interactive_startup.json");

/// Runs `body` in a fresh single-test child whose working directory is a disposable root
/// and whose package directory is `package` inside it, returning its standard error.
pub(super) fn isolated_package(test: &str, body: impl AsyncFnOnce(&Path, &Path)) -> Vec<u8> {
    if maestro_test_process::selection().is_child() {
        let directory = std::env::var(&*ENV_AGENT_DIR).unwrap();
        let package = std::env::var(&*ENV_PACKAGE_DIR).unwrap();
        on_tasks(body(Path::new(&directory), Path::new(&package)));
        return Vec::new();
    }
    let scratch = child::Scratch::new();
    let directory = scratch.0.path();
    let package = directory.join("package");
    std::fs::create_dir(&package).unwrap();
    let output = TestProcess::current(test)
        .unwrap()
        .case("child")
        .nocapture()
        .single_threaded()
        .current_dir(directory)
        .envs([
            (ENV_AGENT_DIR.as_str(), directory.to_str().unwrap()),
            (ENV_PACKAGE_DIR.as_str(), package.to_str().unwrap()),
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    output.stderr
}

/// Runs `body` in a fresh single-test child whose agent directory is a disposable root,
/// so the constructor reads its key bindings from there rather than the developer's.
fn isolated(test: &str, body: impl AsyncFnOnce(&Path)) {
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
    body: impl AsyncFnOnce(&Path),
) {
    if let Some(output) = isolated_output(test, case, envs, "trap '' XFSZ", body) {
        assert!(
            output.status.success(),
            "child probe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Runs `body` as `case` of `test` in the child [`isolated_case`] describes, after `setup`
/// runs in the child's shell, and returns that child's output to the parent; the child
/// itself returns nothing.
pub(super) fn isolated_output(
    test: &str,
    case: &str,
    envs: &[(&str, &str)],
    setup: &str,
    body: impl AsyncFnOnce(&Path),
) -> Option<std::process::Output> {
    let selection = maestro_test_process::selection();
    if selection.is_child() {
        if selection.runs_case(case) {
            let directory = std::env::var(&*ENV_AGENT_DIR).unwrap();
            on_tasks(body(Path::new(&directory)));
        }
        return None;
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
    let output = TestProcess::current(test)
        .unwrap()
        .env_clear()
        .case(case)
        .shell_setup("/bin/bash", setup)
        .temp_dir(root.join("tmp"))
        .nocapture()
        .single_threaded()
        .current_dir(root)
        .envs(all)
        .output()
        .expect("child starts");
    Some(output)
}

/// A controlled replacement for one resource reload.
type ReloadOperation = Box<dyn Fn() -> Pin<Box<dyn Future<Output = io::Result<()>>>>>;

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
    /// A controlled reload operation, when supplied by a witness.
    pub(super) on_reload: RefCell<Option<ReloadOperation>>,
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
            on_reload: RefCell::default(),
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
        self.calls.borrow_mut().push("reload");
        if let Some(reload) = self.on_reload.borrow().as_ref() {
            return reload();
        }
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
    /// The startup offline selection supplied by session services.
    pub(super) offline: Option<bool>,
    /// Supplied terminal ports for native or failing producer witnesses.
    pub(super) terminal: Option<(
        maestro_tui::tui::TerminalHandle,
        Rc<dyn maestro_tui::tui::TuiRuntime>,
    )>,
    /// Controlled native effects, with no real process exit by default.
    pub(super) lifecycle: Rc<dyn super::lifecycle::LifecycleEffects>,
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
    /// Overrides the default replacement-session factory when supplied.
    pub(super) create: Option<CreateAgentSessionRuntimeFactory>,
    /// Starts each replacement with fresh services instead of reusing the current ones.
    pub(super) replacing: bool,
    /// Creates the session history under a file in this directory when supplied.
    pub(super) session_history: Option<std::path::PathBuf>,
    /// The directories the theme registry reads, when it records themes on disk.
    pub(super) directories: Option<maestro_theme::ThemeDirectories>,
    /// Further inline extensions beside `extension`, each under the path given.
    pub(super) further: Vec<(String, ExtensionFactory<Local>)>,
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
    /// The stored credentials the session is built over, in memory when absent.
    pub(super) auth: Option<Arc<AuthStorage>>,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            offline: None,
            terminal: None,
            lifecycle: Rc::new(lifecycle::Effects::default()),
            options: InteractiveModeOptions::default(),
            configure: Box::new(|_| {}),
            select: None,
            cwd: None,
            context: Vec::new(),
            skills: Vec::new(),
            prompts: Vec::new(),
            directories: None,
            extension: None,
            create: None,
            replacing: false,
            session_history: None,
            further: Vec::new(),
            themes: Vec::new(),
            extensions: Vec::new(),
            errors: Vec::new(),
            commands: Vec::new(),
            diagnostics: LoadedDiagnostics::default(),
            watcher: None,
            auth: None,
        }
    }
}

/// Loads `factory` and then `further` as the session's extensions; their initialization
/// must finish at once.
fn loaded(
    cwd: &str,
    factory: ExtensionFactory<Local>,
    further: Vec<(String, ExtensionFactory<Local>)>,
) -> Rc<LoadExtensionsResult<Local>> {
    let local = Rc::new(tokio::task::LocalSet::new());
    let bus = <Local as DefaultEventBus>::default_event_bus(&local);
    loaded_on_bus(cwd, factory, &bus.for_extension(), further)
}

/// Loads synchronous-ready extensions on the supplied shared event bus.
fn loaded_on_bus(
    cwd: &str,
    factory: ExtensionFactory<Local>,
    bus: &maestro_app::EventBus<Local>,
    further: Vec<(String, ExtensionFactory<Local>)>,
) -> Rc<LoadExtensionsResult<Local>> {
    let runtime = create_extension_runtime::<Local>();
    let mut extensions = Vec::new();
    for (path, factory) in std::iter::once((None, factory)).chain(
        further
            .into_iter()
            .map(|(path, factory)| (Some(path), factory)),
    ) {
        let loading = std::pin::pin!(load_extension_from_factory(
            factory,
            cwd.to_owned(),
            bus.clone(),
            Rc::clone(&runtime),
            path,
        ));
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        let std::task::Poll::Ready(owner) = std::future::Future::poll(loading, &mut context) else {
            panic!("the inline extension initializes at once");
        };
        extensions.push(owner.unwrap());
    }
    Rc::new(LoadExtensionsResult {
        extensions,
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
        let extensions = self.extension.map(|init| loaded(&cwd, init, self.further));
        let history = self
            .session_history
            .as_ref()
            .map(|d| d.to_str().unwrap().to_owned());
        let mut result = session::session_with_auth(
            &cwd,
            self.auth,
            move |inner| {
                let loader = Rc::new(ForwardingLoader::new(inner, resources, extensions));
                *created.borrow_mut() = Some(Rc::clone(&loader));
                loader
            },
            history.as_deref(),
        );
        select_offline(&mut result, self.offline);
        let settings = Rc::clone(result.session.settings_manager());
        prepare_settings(settings, self.configure, self.select, &loader);
        let loader = loader.borrow().clone().unwrap();
        let create = self
            .create
            .unwrap_or_else(|| create(Rc::clone(&result.services), self.replacing));
        let host = Rc::new(AgentSessionRuntime::new(result, create));
        let terminal = RecordingTerminal::new(80, 24);
        let runtime = ManualRuntime::new();
        let themes = registry(self.directories);
        let local = Rc::new(tokio::task::LocalSet::new());
        let mode = InteractiveMode::new(
            Rc::clone(&host),
            self.options,
            self.terminal
                .unwrap_or_else(|| (terminal.handle(), runtime.handle())),
            Rc::clone(&themes),
            self.watcher
                .unwrap_or_else(|| Rc::new(NativeFooterOperations::new(local))),
        )
        .unwrap();
        World {
            mode: lifecycle::replace_effects(mode, self.lifecycle),
            terminal,
            runtime,
            themes,
            host,
            loader,
        }
    }
}

/// Initializes the presentation before settling its first frame.
pub(super) async fn start(world: &World) {
    world.mode.init().await.unwrap();
}

/// Applies an explicit startup selection before any runtime aliases the services.
fn select_offline(
    result: &mut maestro_app::CreateAgentSessionRuntimeResult,
    offline: Option<bool>,
) {
    if let Some(offline) = offline {
        Rc::get_mut(&mut result.services).unwrap().offline = Some(offline);
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
        // The replacement serves the session manager the runtime opened for it.
        std::mem::swap(
            &mut *replacement.session.session_manager().borrow_mut(),
            &mut *options.session_manager.borrow_mut(),
        );
        Box::pin(async { Ok(replacement) })
    })
}

/// Builds a default mode in `directory` and initializes it, settling the first frame.
async fn started(directory: &Path) -> World {
    started_with(directory, None).await
}

/// Builds a mode over `auth`, or over an empty in-memory store, and initializes it,
/// settling the first frame.
pub(super) async fn started_with(directory: &Path, auth: Option<Arc<AuthStorage>>) -> World {
    let world = Setup {
        auth,
        ..Setup::default()
    }
    .build(directory);
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    world
}

/// The published theme's escape for each recorded role marker.
pub(super) fn roles(theme: &Theme, text: &str) -> String {
    let mut text = text.replace("<version>", VERSION);
    for (marker, role) in [
        ("@border@", ThemeColor::Border),
        ("@accent@", ThemeColor::Accent),
        ("@border@", ThemeColor::Border),
        ("@borderMuted@", ThemeColor::BorderMuted),
        ("@text@", ThemeColor::Text),
        ("@userMessageText@", ThemeColor::UserMessageText),
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
        ("@mdCodeBlock@", ThemeColor::MdCodeBlock),
        ("@mdCodeBlockBorder@", ThemeColor::MdCodeBlockBorder),
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
    /// Complete macOS layout when its key names differ.
    expected_macos: Option<serde_json::Value>,
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
                serde_json::from_value(if cfg!(target_os = "macos") {
                    case.expected_macos.unwrap_or(case.expected)
                } else {
                    case.expected
                })
                .unwrap(),
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
        async |directory| {
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
                start(&world).await;
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
        async |directory| {
            for (input, expected) in cases::<Input, Expected>(
                FIXTURES,
                "startup_notices_keep_labels_styles_padding_and_redraw",
            ) {
                let (Input::Notice { kind, text, width }, Expected::Notice { lines, renders }) =
                    (input, expected)
                else {
                    panic!("a notice case");
                };
                let world = started(directory).await;
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
        async |directory| {
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
            let world = started(directory).await;
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
        _cursor: tui_autocomplete::CursorPosition,
        _options: tui_autocomplete::CompletionOptions<'a, Self::Signal>,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<
                        Option<AutocompleteSuggestions>,
                        tui_autocomplete::CompletionError,
                    >,
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
        _cursor: tui_autocomplete::CursorPosition,
        item: &AutocompleteItem,
        _prefix: &str,
    ) -> tui_autocomplete::CompletionResult {
        tui_autocomplete::CompletionResult {
            lines: vec![item.value.clone()],
            cursor_line: 0,
            cursor_col: item.value.len(),
        }
    }
}

/// The candidate rows the mode's editor shows with `maximum` configured.
async fn visible_candidates(directory: &Path, maximum: f64) -> usize {
    let world = Setup {
        configure: Box::new(move |settings| settings.set_autocomplete_max_visible(maximum)),
        ..Setup::default()
    }
    .build(directory);
    start(&world).await;
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
async fn editor_indent(directory: &Path, padding: f64) -> usize {
    let world = Setup {
        configure: Box::new(move |settings| settings.set_editor_padding_x(padding)),
        ..Setup::default()
    }
    .build(directory);
    start(&world).await;
    world.mode.editor.set_text("typed");
    let rows = world.mode.editor.render(40);
    let row = rows.iter().find(|row| row.contains("typed")).unwrap();
    row.len() - row.trim_start_matches(' ').len()
}

#[test]
fn startup_constructor_forwards_each_display_setting() {
    isolated(
        "startup_constructor_forwards_each_display_setting",
        async |directory| {
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
                start(&world).await;
                world.runtime.settle().unwrap();
                let visibility = world
                    .terminal
                    .writes()
                    .into_iter()
                    .rev()
                    .find(|write| write == "\x1b[?25h" || write == "\x1b[?25l");
                let expected = if cursor { "\x1b[?25h" } else { "\x1b[?25l" };
                assert_eq!(visibility.as_deref(), Some(expected), "terminal cursor");
                world.mode.show_error("tall").unwrap();
                world.runtime.settle().unwrap();
                let redraws = world.mode.ui.full_redraws();
                world.mode.chat_container.clear();
                world.mode.ui.request_render(false);
                world.runtime.settle().unwrap();
                assert_eq!(world.mode.ui.full_redraws() > redraws, shrink);
            }
            let mut indents = Vec::new();
            for padding in [0.0, 1.0, 2.0] {
                indents.push(editor_indent(directory, padding).await);
            }
            assert_eq!(indents, [0, 1, 2]);
            let mut visible = Vec::new();
            for maximum in [3.0, 5.0, 20.0] {
                visible.push(visible_candidates(directory, maximum).await);
            }
            assert_eq!(visible, [3, 5, 20]);
            for enabled in [true, false] {
                let world = Setup {
                    configure: Box::new(move |settings| settings.set_compaction_enabled(enabled)),
                    ..Setup::default()
                }
                .build(directory);
                start(&world).await;
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
        async |directory| {
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
    isolated(
        "startup_layout_focus_and_start_happen_once",
        async |directory| {
            let world = started(directory).await;
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
            assert_eq!(mode.widget_container_above.render(80), [""]);
            assert!(mode.widget_container_below.render(80).is_empty());
            world.terminal.send_input("x");
            assert_eq!(mode.editor.get_text(), "x");
            mode.init().await.unwrap();
            assert!(same(&mode.ui.children().borrow()));
            assert_eq!(world.terminal.events(), ["start"]);
            assert_eq!(mode.header_container.children().borrow().len(), 3);
        },
    );
}

#[test]
fn startup_failed_terminal_start_does_not_mark_initialized() {
    isolated(
        "startup_failed_terminal_start_does_not_mark_initialized",
        async |directory| {
            let world = Setup::default().build(directory);
            world
                .terminal
                .set_hide_error(Some(io::ErrorKind::BrokenPipe));
            let error = world.mode.init().await.unwrap_err();
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
        async |directory| {
            let world = started(directory).await;
            let header = world.mode.header_container.render(80);
            let footer = world.mode.footer.render(80);
            let level = thinking_text(&world.host.session().thinking_level()).0;
            let dark = world.themes.theme().get().unwrap();
            let replaced = (dark.get_thinking_border_color(level))("─");
            let published = light();
            let border = (published.get_thinking_border_color(level))("─");
            assert_ne!(replaced, border);
            world.themes.set_theme_instance(published).unwrap();
            assert_eq!(world.runtime.pending(), 1);
            world.terminal.clear_writes();
            world.runtime.settle().unwrap();
            assert!(!world.terminal.writes().is_empty());
            assert_eq!(world.mode.header_container.render(80), header);
            assert_ne!(world.mode.footer.render(80), footer);
            let width = world.terminal.size().0;
            let editor = world.mode.editor.render(width);
            assert!(
                editor[0].contains(&border),
                "the editor border uses the new theme"
            );
            assert!(
                !editor[0].contains(&replaced),
                "no border keeps the old theme"
            );
            let painted = world.terminal.writes().concat();
            for row in editor.iter().chain(&world.mode.footer.render(width)) {
                // The frame writer removes the hardware-cursor marker before it writes a row.
                let row = row.replacen(maestro_tui::CURSOR_MARKER, "", 1);
                assert!(painted.contains(&row), "painted row {row:?}");
            }
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
        async |directory| {
            let World {
                mode,
                terminal,
                runtime,
                themes,
                host,
                loader,
            } = started(directory).await;
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

use super::bash::ShellOperations;

/// The failures the mode's work reported.
pub(super) type Failures = Rc<RefCell<Vec<Box<dyn Error>>>>;

/// Runs the work the mode handed its terminal runtime beside `body`, collecting the
/// failures that work reported.
pub(super) async fn driving<T>(
    world: &World,
    body: impl Future<Output = T>,
) -> (T, Vec<Box<dyn Error>>) {
    let runtime = world.runtime.clone();
    let finished = Rc::new(Cell::new(false));
    let errors: Failures = Rc::default();
    let driver = {
        let finished = Rc::clone(&finished);
        let errors = Rc::clone(&errors);
        tokio::task::spawn_local(async move { drive(&runtime, &finished, &errors).await })
    };
    let value = body.await;
    finished.set(true);
    driver.await.unwrap();
    let errors = Rc::try_unwrap(errors).unwrap().into_inner();
    (value, errors)
}

/// Drives the mode's futures until the caller reports it is done.
async fn drive(runtime: &ManualRuntime, finished: &Rc<Cell<bool>>, errors: &Failures) {
    loop {
        dispatch(runtime, errors);
        if finished.get() {
            return;
        }
        tokio::task::yield_now().await;
    }
}

/// Hands every future the mode admitted to the task set running beside the caller.
fn dispatch(runtime: &ManualRuntime, errors: &Failures) {
    for future in runtime.take_futures() {
        let errors = Rc::clone(errors);
        tokio::task::spawn_local(async move { await_work(future, &errors).await });
    }
}

/// Awaits one future the mode admitted, recording its failure.
async fn await_work(future: maestro_tui::tui::LocalFuture, errors: &Failures) {
    if let Err(error) = future.await {
        errors.borrow_mut().push(error);
    }
}

/// Runs `body` inside a current-thread runtime and task set, where the mode's work and the
/// session's own tasks make progress.
pub(super) fn on_tasks<T>(body: impl Future<Output = T>) -> T {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, body)
}

/// Submits `text` through the editor's installed submission.
pub(super) fn submit(mode: &Rc<InteractiveMode>, text: &str) {
    mode.editor.on_submit().unwrap()(text);
}

/// Awaits the work handed to `runtime` last, by the input just given, and returns its failure.
///
/// Work handed over earlier goes back to `runtime` unawaited, so a [`driving`] task beside the
/// caller runs it: such work, like a running shell command's output, can end only after the
/// caller's later steps.
pub(super) async fn admitted(runtime: &ManualRuntime) -> Vec<Box<dyn Error>> {
    let mut work = runtime.take_futures();
    let command = work.pop().expect("the input handed over its work");
    for earlier in work {
        maestro_tui::tui::TuiRuntime::spawn_local(runtime, earlier);
    }
    command.await.err().into_iter().collect()
}

/// Waits until `path` exists, yielding so the work that creates it runs.
pub(super) async fn until_exists(path: &Path) {
    for _ in 0..10_000 {
        if path.exists() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("{} appeared", path.display());
}

/// Waits until no shell row is producing output and the transcript holds `rows` children.
pub(super) async fn until_complete(world: &World, rows: usize) {
    for _ in 0..10_000 {
        if active(world).is_none() && world.mode.chat_container.children().borrow().len() == rows {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!(
        "the shell command completed: active={} rows={}",
        active(world).is_some(),
        world.mode.chat_container.children().borrow().len()
    );
}

/// The shell row that is still producing output, when one is attached.
pub(super) fn active(world: &World) -> Option<Rc<BashExecutionComponent>> {
    world.mode.active_shell.borrow().clone()
}

/// Writes an executable shell script answering every command with `body`.
pub(super) fn scripted(directory: &Path, name: &str, body: &str) -> String {
    use std::os::unix::fs::PermissionsExt;
    let path = directory.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path.to_str().unwrap().to_owned()
}

/// A response the controlled provider left open.
pub(super) struct Held {
    /// The open response.
    pub(super) stream: AssistantMessageEventStream,
    /// The turn's cancellation.
    pub(super) signal: Cancellation,
}

impl Held {
    /// How many times the turn was stopped.
    pub(super) fn aborts(&self) -> usize {
        usize::from(self.signal.is_aborted())
    }
}

/// A finished answer of the controlled provider.
pub(super) fn answer() -> Arc<RwLock<AssistantMessage>> {
    Arc::new(RwLock::new(AssistantMessage {
        content: Vec::new(),
        api: "unknown".into(),
        provider: "unknown".into(),
        model: "unknown".into(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage::default(),
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: 0.0,
    }))
}

/// Points the session's provider at an open response: the next prompt receives it and the
/// receiver yields it with its cancellation; a later prompt answers at once.
fn held_response(world: &World) -> tokio::sync::oneshot::Receiver<Held> {
    let session = world.mode.runtime_host.session();
    world
        .host
        .services()
        .auth_storage
        .set_runtime_api_key("unknown", "key");
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let sender = RefCell::new(Some(sender));
    *session.agent().stream_fn.write().unwrap() = Some(Rc::new(move |_, _, options, _| {
        let stream = AssistantMessageEventStream::new();
        match sender.borrow_mut().take() {
            Some(sender) => drop(sender.send(Held {
                stream: stream.clone(),
                signal: options.common.signal.unwrap(),
            })),
            None => stream.end(Some(answer())),
        }
        Box::pin(std::future::ready(Ok(stream)))
    }));
    receiver
}

/// Runs `body` on the started `world` while its session streams a response the controlled
/// provider holds open, giving it that response; then ends the response and waits for the
/// prompt to settle.
pub(super) async fn with_held_response<T>(world: &World, body: impl AsyncFnOnce(&Held) -> T) -> T {
    let session = world.mode.runtime_host.session();
    let receiver = held_response(world);
    let prompted = Rc::clone(&session);
    let run =
        tokio::task::spawn_local(
            async move { prompted.prompt("run", PromptOptions::default()).await },
        );
    let held = receiver.await.unwrap();
    assert!(session.is_streaming());
    let value = body(&held).await;
    held.stream.end(Some(answer()));
    run.await.unwrap().unwrap();
    value
}

/// Awaits `body` on the started `world` while its session streams a response
/// the controlled provider holds open; the response ends afterwards.
pub(super) async fn streaming<T>(world: &World, body: impl AsyncFnOnce(&World, &Held) -> T) -> T {
    with_held_response(world, async |held| {
        world.mode.chat_container.clear();
        body(world, held).await
    })
    .await
}

/// Runs `body` on the started `world` inside the caller's task set while its session
/// streams a response the controlled provider holds open, handing `body` the response's
/// cancellation; the response ends afterwards.
pub(super) async fn holding<T, F>(world: &World, body: impl FnOnce(Cancellation) -> F) -> T
where
    F: Future<Output = T>,
{
    with_held_response(world, async |held| body(held.signal.clone()).await).await
}

/// The editor's top border row as the recorder's painted probe glyph describes it.
pub(super) fn border_row(world: &World, recorded: &str) -> String {
    let theme = world.themes.theme().get().unwrap();
    roles(&theme, &recorded.replace('━', "─")).repeat(80)
}

/// The editor's rendered top border row.
pub(super) fn rendered_border(world: &World) -> String {
    world.mode.editor.render(80).swap_remove(0)
}

/// Selects `level` on the session as its agent state, whatever the model supports.
pub(super) fn select_level(world: &World, level: maestro_app::ThinkingLevel) {
    world
        .host
        .session()
        .agent()
        .state()
        .write()
        .unwrap()
        .thinking_level = level;
}

/// A started mode whose shell prints an output-only marker for the command it runs.
pub(super) async fn echoing_shell(directory: &Path) -> World {
    let script = scripted(directory, "echoing", "printf 'printed %s\\n' \"$2\"");
    bash::shelling(directory, &script).await
}

/// Submits `count` shell commands, each with its own text, while the response streams,
/// awaiting each command it admits so its row settles.
pub(super) async fn start_shells(world: &World, count: usize) {
    start_commands(world, count, |index| format!("!mark{index}")).await;
}

/// Submits `count` shell commands worded by `text` while the response streams, awaiting
/// each command it admits so its row settles.
pub(super) async fn start_commands(world: &World, count: usize, text: impl Fn(usize) -> String) {
    for index in 0..count {
        submit(&world.mode, &text(index));
        for command in world.runtime.take_futures() {
            command.await.expect("the shell command completes");
        }
        assert_eq!(
            world.mode.pending_shells.borrow().len(),
            index + 1,
            "shell row {index} settles"
        );
    }
}

/// The positions of `rows` among the transcript's children, in the order they occur.
pub(super) fn transcript_positions(
    world: &World,
    rows: &[Rc<BashExecutionComponent>],
) -> Vec<usize> {
    let children = world.mode.chat_container.children();
    let addresses: Vec<usize> = children.borrow().iter().map(component_address).collect();
    rows.iter()
        .filter_map(|row| {
            let address = component_address(row);
            addresses.iter().position(|shown| *shown == address)
        })
        .collect()
}

mod dispatch;

/// Footer and theme-watch effects where only the theme watches are observed.
#[derive(Default)]
struct Watched {
    /// Whether each theme watch is still open, in creation order.
    open: RefCell<Vec<Rc<Cell<bool>>>>,
    /// The listener of each theme watch, in creation order.
    listeners: RefCell<Vec<WatchListener>>,
    /// The debounced reload the latest watch timer holds.
    reload: WatchReload,
}

impl Watched {
    /// Deliver `changed` to the latest watch's listener, as the watched directory would.
    fn announce(&self, changed: Option<&str>) {
        let listener = self
            .listeners
            .borrow()
            .last()
            .cloned()
            .expect("a theme watch is open");
        listener(changed.map(str::to_owned));
    }

    /// Run the reload the latest watch timer retained, as the debounce would.
    fn reload(&self) {
        let reload = self.reload.borrow_mut().take();
        if let Some(reload) = reload {
            reload();
        }
    }
}

/// One open theme watch.
struct Handle {
    /// Set once the watch closes.
    closed: Rc<Cell<bool>>,
}

/// The listener of one open theme watch.
type WatchListener = Rc<dyn Fn(Option<String>)>;

impl ThemeWatcher for Handle {
    fn close(&mut self) -> io::Result<()> {
        self.closed.set(true);
        Ok(())
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.closed.set(true);
    }
}

/// The debounced reload a theme watch timer retains.
type WatchReload = Rc<RefCell<Option<Box<dyn FnOnce()>>>>;

/// A scheduled theme reload, kept until its watch is cancelled or the test runs it.
struct Pending {
    /// The reload this timer owns.
    reload: WatchReload,
}

impl ThemeReloadTimer for Pending {
    fn cancel(&mut self) {
        self.reload.borrow_mut().take();
    }
}

impl ThemeWatchOperations for Watched {
    fn watch(
        &self,
        _path: &str,
        listener: WatchListener,
        _on_error: Rc<dyn Fn()>,
    ) -> io::Result<Box<dyn ThemeWatcher>> {
        let closed = Rc::new(Cell::new(false));
        self.open.borrow_mut().push(Rc::clone(&closed));
        self.listeners.borrow_mut().push(listener);
        Ok(Box::new(Handle { closed }))
    }

    fn schedule(&self, _delay: Duration, callback: Box<dyn FnOnce()>) -> Box<dyn ThemeReloadTimer> {
        *self.reload.borrow_mut() = Some(callback);
        Box::new(Pending {
            reload: Rc::clone(&self.reload),
        })
    }
}

impl FooterOperations for Watched {
    fn symbolic_ref_async(
        &self,
        _repo_dir: &str,
        callback: Box<dyn FnOnce(io::Result<Option<String>>)>,
    ) {
        callback(Ok(None));
    }

    fn watch_file(
        &self,
        _path: &str,
        _interval: Duration,
        _listener: Rc<dyn Fn(FooterFileStats, FooterFileStats)>,
    ) -> Box<dyn ThemeWatcher> {
        Box::new(Handle {
            closed: Rc::new(Cell::new(false)),
        })
    }

    fn symbolic_ref_sync(&self, _repo_dir: &str) -> io::Result<Option<String>> {
        Ok(None)
    }
}

/// A mode whose selected custom theme is watched through `watched`, which must first see
/// one open watch.
fn watching(directory: &Path, watched: &Rc<Watched>) -> World {
    let custom = directory.join("themes");
    std::fs::create_dir_all(&custom).unwrap();
    // The watcher only needs the entry's file; the registered theme answers the load.
    std::fs::write(custom.join("watched.json"), "{}").unwrap();
    Setup {
        configure: Box::new(|settings| settings.set_theme("watched".to_owned())),
        themes: vec![published::named(1, Some("watched"))],
        directories: Some(maestro_theme::ThemeDirectories {
            themes_dir: String::new(),
            custom_themes_dir: custom.to_str().unwrap().to_owned(),
        }),
        watcher: Some(Rc::clone(watched) as Rc<dyn FooterOperations>),
        ..Setup::default()
    }
    .build(directory)
}

#[test]
fn fatal_runtime_error_writes_its_notice_before_stopping_the_terminal() {
    isolated(
        "fatal_runtime_error_writes_its_notice_before_stopping_the_terminal",
        async |directory| {
            for pending in [0, 262_144, 262_145] {
                let watched = Rc::new(Watched::default());
                let world = watching(directory, &watched);
                start(&world).await;
                world.runtime.settle().unwrap();
                assert_eq!(watched.open.borrow().len(), 1);
                assert!(!watched.open.borrow()[0].get());
                world.terminal.set_pending_output_bytes(pending);
                world.terminal.clear_writes();
                world.mode.report_fatal_runtime_error(
                    "Failed to create session",
                    &io::Error::other("boom"),
                );
                let theme = world.themes.theme().get().unwrap();
                let notice = theme
                    .fg(&ThemeColor::Error, "Error: Failed to create session: boom")
                    .unwrap();
                let rows = world.mode.chat_container.render(100);
                assert!(
                    rows.iter().any(|row| row.contains(&notice)),
                    "the failure notice: {rows:?}"
                );
                let writes = world.terminal.writes();
                let notice_index = writes
                    .iter()
                    .position(|write| {
                        strip_ansi(write)
                            .unwrap()
                            .contains("Error: Failed to create session: boom")
                    })
                    .unwrap_or_else(|| {
                        panic!("the failure notice reached the terminal at {pending}: {writes:?}")
                    });
                let restore_index = writes
                    .iter()
                    .position(|write| write.contains("\x1b[?25h"))
                    .expect("the cursor is shown");
                assert!(
                    notice_index < restore_index,
                    "the notice precedes the restore at {pending}: {writes:?}"
                );
                assert!(watched.open.borrow()[0].get(), "the theme watch closes");
                assert_eq!(world.terminal.events(), ["start", "stop"]);
            }
        },
    );
}

#[test]
fn fatal_runtime_error_ends_the_process_with_failure_status() {
    let test = "fatal_runtime_error_ends_the_process_with_failure_status";
    let selection = maestro_test_process::selection();
    if selection.is_child() {
        if selection.runs_case("fatal") {
            let directory = std::env::var(&*ENV_AGENT_DIR).unwrap();
            let world = Setup::default().build(Path::new(&directory));
            println!("\nPROBE fatal body started");
            world
                .mode
                .handle_fatal_runtime_error("Failed to create session", &io::Error::other("boom"));
        }
        return;
    }
    let scratch = child::Scratch::new();
    let output = TestProcess::current(test)
        .unwrap()
        .case("fatal")
        .current_dir(scratch.0.path())
        .env(&*ENV_AGENT_DIR, scratch.0.path())
        .nocapture()
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("PROBE fatal body started"),
        "the child reached the fatal call: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(output.status.code(), Some(1));
}

/// Seeds retained rows, queued text and an active stream from the replaced session.
fn seed_replaced_session(world: &World) {
    let stale =
        crate::ToolExecutionComponent::new(transcript::tool_params(world, "stale")).unwrap();
    world
        .mode
        .pending_tools
        .borrow_mut()
        .insert("stale".to_owned(), Rc::new(stale));
    world
        .mode
        .chat_container
        .add_child(Rc::new(Text::new("STALE".to_owned(), 0, 0, None)));
    world
        .mode
        .pending_messages_container
        .add_child(Rc::new(Text::new("STALE".to_owned(), 0, 0, None)));
    world.mode.compaction_queued_messages.borrow_mut().push(
        super::queues::CompactionQueuedMessage {
            text: "STALE QUEUE".to_owned(),
            mode: maestro_app::messages::UserMessageDelivery::FollowUp,
        },
    );
    events::dispatch(
        world,
        maestro_app::AgentEvent::MessageStart {
            message: transcript::message(transcript::assistant(
                &json!([{"type": "text", "text": "STALE STREAM"}]),
                "stop",
                None,
            )),
        },
    );
    assert!(world.mode.streaming.borrow().is_some());
}

#[test]
fn current_session_state_rendering_clears_stale_rows_and_replays_the_session() {
    isolated(
        "current_session_state_rendering_clears_stale_rows_and_replays_the_session",
        async |directory| {
            let world = started(directory).await;
            let session = world.host.session();
            let mut manager = session.session_manager().borrow_mut();
            manager
                .append_message(transcript::user_record("REPLAYED"))
                .unwrap();
            manager
                .append_message(transcript::assistant(
                    &json!([transcript::call("call", "unknown_tool")]),
                    "toolUse",
                    None,
                ))
                .unwrap();
            drop(manager);
            seed_replaced_session(&world);
            let shell = Rc::new(BashExecutionComponent::new(
                "old".to_owned(),
                maestro_tui::TUI::clone(&world.mode.ui),
                world.themes.theme(),
                false,
            ));
            world
                .mode
                .pending_shells
                .borrow_mut()
                .push(Rc::clone(&shell));
            *world.mode.active_shell.borrow_mut() = Some(shell);
            world.mode.render_current_session_state().unwrap();
            assert_eq!(
                (
                    world.mode.compaction_queued_messages.borrow().is_empty(),
                    world.mode.streaming.borrow().is_none(),
                    world.mode.pending_shells.borrow().is_empty(),
                    active(&world).is_none(),
                ),
                (true, true, true, true),
                "the replaced session's queued input, stream and shell rows are discarded"
            );
            let rows = world.mode.chat_container.render(100);
            assert!(!rows.iter().any(|row| row.contains("STALE")), "{rows:?}");
            assert!(rows.iter().any(|row| row.contains("REPLAYED")), "{rows:?}");
            assert!(
                world
                    .mode
                    .pending_messages_container
                    .children()
                    .borrow()
                    .is_empty()
            );
            let pending: Vec<_> = world.mode.pending_tools.borrow().keys().cloned().collect();
            assert_eq!(pending, ["call"]);
            maestro_tui::tui::InputHandler::handle_input(world.mode.editor.as_ref(), "\x1b[A");
            assert_eq!(world.mode.editor.get_text(), "REPLAYED");
        },
    );
}
