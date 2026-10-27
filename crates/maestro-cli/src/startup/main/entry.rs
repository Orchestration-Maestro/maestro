//! The command-line startup entry over the existing application owners.
use super::bootstrap::{
    AppMode, collect_settings_diagnostics, report_diagnostics, report_parser_diagnostics,
    resolve_app_mode, resolve_session_dir, takes_over_stdout, validate_fork_flags,
};
use super::dispatch::{DispatchOutput, ModeDispatch, dispatch_mode};
use super::factory::{CliPaths, RuntimeInputs, runtime_factory};
use super::inputs::{prepare_initial_message, read_piped_stdin};
use super::presentation::MissingSessionCwdPresentation;
use super::selected_sessions::{
    SessionDirs, SessionEffects, SessionPresentations, create_session_manager,
};
use super::writers::SharedWriter;
use crate::input::args::{Args, ListModels, parse_args, print_help};
use crate::input::config_selector::ConfigPresentation;
use crate::input::file_processor::FileArgumentOperations;
use crate::input::initial_message::InitialMessageResult;
use crate::input::list_models::{ListModelsOptions, list_models};
use crate::input::session_picker::SessionPresentation;
use crate::packages::package_manager_cli::presentation::line;
use crate::packages::{PackageCommandContext, handle_config_command, handle_package_command};
use maestro_app::configuration::migrations::{
    MigrationResult, MigrationStyle, run_migrations, show_deprecation_warnings,
};
use maestro_app::configuration::{config, timings::Timings, version_check::VersionCheckOptions};
use maestro_app::presentation_data::mode_dispatch::{InteractiveModeOptions, ModeLaunchers};
use maestro_app::{
    AgentSessionRuntime, AuthStorage, CreateAgentSessionRuntimeOptions, ExtensionFactory, Local,
    ModuleLoader, ReadonlySessionManager, SessionManager, create_agent_session_runtime,
    is_truthy_env_flag, output_guard,
};
use maestro_theme::{Collator, Theme};
use std::{cell::RefCell, error::Error, io, ops::ControlFlow, rc::Rc, sync::Arc};
use tokio::{io::AsyncBufRead, task::LocalSet};

/// A startup phase's result: continue with a value, or end startup with a status.
type Phase<T> = Result<ControlFlow<i32, T>, Box<dyn Error>>;

/// Extra extension factories supplied to the command-line entry.
#[derive(Default)]
pub struct MainOptions {
    /// Factories loaded after the discovered extensions, in their supplied order.
    pub extension_factories: Vec<ExtensionFactory<Local>>,
}

/// Exports a session file to an optional destination, returning the written path.
///
/// It is the caller's existing file-export operation, not a second serializer.
pub type MainExport = Rc<dyn Fn(&str, Option<&str>) -> Result<String, Box<dyn Error>>>;

/// The existing application and presentation effects one startup uses.
///
/// Borrowed input lives for `'input`; every other borrowed effect lives for `'effects`.
/// Only owned handles move into the retained runtime factory.
pub struct MainHost<'input, 'effects> {
    /// Package operations, directory observations and the shared text streams.
    pub package: PackageCommandContext<'effects>,
    /// Installs the caller's native defaults for the startup offline selection, once.
    pub apply_startup: Rc<dyn Fn(bool)>,
    /// The configuration presentation, whose theme and watcher startup also uses.
    pub config: ConfigPresentation,
    /// The resume selector presentation.
    pub sessions: SessionPresentation,
    /// The missing-directory confirmation presentation.
    pub missing_cwd: MissingSessionCwdPresentation,
    /// The local task set driving extension work.
    pub local: Rc<LocalSet>,
    /// The executable extension module adapter.
    pub module_loader: ModuleLoader<Local>,
    /// The interactive and RPC presentations.
    pub modes: ModeLaunchers,
    /// Standard input: confirmation answers and piped prompt text.
    pub stdin: &'input mut (dyn AsyncBufRead + Unpin),
    /// Whether standard input is a terminal, observed again when it is read.
    pub stdin_is_tty: &'effects dyn Fn() -> bool,
    /// The clock of the startup timing samples, in milliseconds.
    pub now: &'effects dyn Fn() -> f64,
    /// File and image preparation of `@file` arguments.
    pub files: &'effects dyn FileArgumentOperations,
    /// The session export operation.
    pub export_from_file: MainExport,
}

/// The parsed run and the mode it presents in.
struct Startup {
    /// The parsed arguments.
    parsed: Args,
    /// The selected presentation.
    app_mode: AppMode,
}

/// The created runtime and the startup inputs it does not retain.
struct Prepared {
    /// The active runtime.
    runtime: Rc<AgentSessionRuntime>,
    /// The choices every replacement reuses.
    parsed: Rc<Args>,
    /// The prompt queue and file arguments startup consumes itself.
    initial: Args,
    /// The model listing comparator.
    collator: Rc<Collator>,
}

/// Run startup and return the status the process ends with.
///
/// Package and configuration commands run first. Otherwise the arguments are parsed,
/// the session is selected, the runtime is created and the selected mode runs. Early
/// exits return their status instead of ending the process.
///
/// # Errors
/// Returns the first application, presentation, input or output failure reached.
pub async fn main(
    args: Vec<String>,
    options: MainOptions,
    host: MainHost<'_, '_>,
) -> Result<i32, Box<dyn Error>> {
    let mut timings = Timings::new(std::env::var("MAESTRO_TIMING").ok().as_deref(), host.now);
    timings.reset_timings(host.now);
    let offline = args.iter().any(|arg| arg == "--offline")
        || is_truthy_env_flag(std::env::var("MAESTRO_OFFLINE").ok().as_deref());
    (host.apply_startup)(offline);
    let original = host.package.version_options;
    let version_options = VersionCheckOptions {
        timeout_ms: original.timeout_ms,
        fetch: original.fetch.clone(),
        offline: if offline {
            Some(true)
        } else {
            original.offline
        },
        skip_version_check: if offline {
            Some(true)
        } else {
            original.skip_version_check
        },
    };
    let package = PackageCommandContext {
        version_options: &version_options,
        ..host.package
    };
    run_entry(
        args,
        options,
        MainHost { package, ..host },
        timings,
        offline,
    )
    .await
}

/// Run the commands, then every startup phase in order.
async fn run_entry(
    args: Vec<String>,
    options: MainOptions,
    mut host: MainHost<'_, '_>,
    mut timings: Timings,
    offline: bool,
) -> Result<i32, Box<dyn Error>> {
    if handle_package_command(&args, &host.package).await? {
        return Ok(host.package.exit_code.get().unwrap_or(0));
    }
    let config = ConfigPresentation {
        host: host.config.host.clone(),
        theme: Rc::clone(&host.config.theme),
        watcher: Rc::clone(&host.config.watcher),
        presenter: Rc::clone(&host.config.presenter),
    };
    if handle_config_command(&args, &host.package, config).await? {
        return Ok(0);
    }
    let startup = match parse_startup(&args, &host, &mut timings)? {
        ControlFlow::Continue(startup) => startup,
        ControlFlow::Break(status) => return Ok(status),
    };
    let migrations = migrate(&host)?;
    timings.time("runMigrations", host.now);
    let (manager, dirs) = match select_session(&startup, &mut host).await? {
        ControlFlow::Continue(selected) => selected,
        ControlFlow::Break(status) => return Ok(status),
    };
    timings.time("createSessionManager", host.now);
    let mut parsed = startup.parsed;
    let prepared = prepare_runtime(
        &mut parsed,
        manager,
        &dirs,
        options,
        (&host, offline, &mut timings),
    )
    .await?;
    if show_metadata(&prepared, &host)? {
        return Ok(0);
    }
    finish(prepared, host, (startup.app_mode, migrations), &mut timings).await
}

/// Parse the arguments, sample the parse once its diagnostics pass, select the mode, then
/// run the exits that need no session.
fn parse_startup(
    args: &[String],
    host: &MainHost<'_, '_>,
    timings: &mut Timings,
) -> Phase<Startup> {
    let parsed = parse_args(args);
    let styles = host.package.styles_enabled;
    let stderr = &host.package.stderr;
    if let Some(status) =
        report_parser_diagnostics(&parsed.diagnostics, &mut *stderr.borrow_mut(), styles)?
    {
        return Ok(ControlFlow::Break(status));
    }
    timings.time("parseArgs", host.now);
    let app_mode = resolve_app_mode(&parsed, (host.stdin_is_tty)());
    if takes_over_stdout(app_mode) {
        output_guard::take_over_stdout();
    }
    if parsed.version == Some(true) {
        line(&host.package.stdout, maestro_app::VERSION)?;
        return Ok(ControlFlow::Break(0));
    }
    if let Some(source) = &parsed.export {
        return Ok(ControlFlow::Break(export(source, &parsed, host)?));
    }
    if app_mode == AppMode::Rpc && !parsed.file_args.is_empty() {
        let message = "Error: @file arguments are not supported in RPC mode";
        line(stderr, &Theme::red(message, styles))?;
        return Ok(ControlFlow::Break(1));
    }
    if let Some(status) = validate_fork_flags(&parsed, &mut *stderr.borrow_mut(), styles)? {
        return Ok(ControlFlow::Break(status));
    }
    Ok(ControlFlow::Continue(Startup { parsed, app_mode }))
}

/// Export the session to the first positional destination and return the status.
fn export(source: &str, parsed: &Args, host: &MainHost<'_, '_>) -> io::Result<i32> {
    let destination = parsed.messages.front().map(String::as_str);
    match (host.export_from_file)(source, destination) {
        Ok(path) => {
            line(&host.package.stdout, &format!("Exported to: {path}"))?;
            Ok(0)
        }
        Err(error) => {
            let message = Theme::red(&format!("Error: {error}"), host.package.styles_enabled);
            line(&host.package.stderr, &message)?;
            Ok(1)
        }
    }
}

/// Run the startup conversions over the original working directory.
fn migrate(host: &MainHost<'_, '_>) -> io::Result<MigrationResult> {
    let cwd = (host.package.cwd)()?;
    run_migrations(&cwd, &mut migration_output(host))
}

/// Write one conversion notice to ordinary output in its style.
fn migration_output<'a>(
    host: &'a MainHost<'_, '_>,
) -> impl FnMut(MigrationStyle, &str) -> io::Result<()> + 'a {
    let styles = host.package.styles_enabled;
    move |style, text| {
        let notice = match style {
            MigrationStyle::Success => Theme::green(text, styles),
            MigrationStyle::Warning => Theme::yellow(text, styles),
            MigrationStyle::Dim => Theme::dim(text, styles),
            MigrationStyle::Plain => text.to_owned(),
        };
        line(&host.package.stdout, &notice)
    }
}

/// Load the startup settings, then select the session and its working directory.
async fn select_session(
    startup: &Startup,
    host: &mut MainHost<'_, '_>,
) -> Phase<(SessionManager, SessionDirs)> {
    let cwd = (host.package.cwd)()?;
    let agent_dir = (host.package.agent_dir)()?;
    let settings = (host.package.load_settings)(&cwd, &agent_dir)?;
    let diagnostics =
        collect_settings_diagnostics(&mut settings.borrow_mut(), "startup session lookup");
    let styles = host.package.styles_enabled;
    report_diagnostics(&diagnostics, &mut *host.package.stderr.borrow_mut(), styles)?;
    let environment = std::env::var(&*maestro_app::ENV_SESSION_DIR).ok();
    let session_dir = resolve_session_dir(
        &startup.parsed,
        environment.as_deref(),
        &settings.borrow(),
        maestro_app::home_directory,
    )?;
    let dirs = SessionDirs {
        cwd,
        agent_dir,
        session_dir,
    };
    let effects = SessionEffects {
        settings,
        theme: Rc::clone(&host.config.theme),
        watcher: Rc::clone(&host.config.watcher),
        stdin: &mut *host.stdin,
        stdout: Rc::clone(&host.package.stdout),
        stderr: Rc::clone(&host.package.stderr),
        styles,
    };
    let presentations = SessionPresentations {
        sessions: SessionPresentation {
            host: host.sessions.host.clone(),
            keybindings: host.sessions.keybindings.clone(),
            presenter: Rc::clone(&host.sessions.presenter),
        },
        missing_cwd: MissingSessionCwdPresentation {
            create_ui: Rc::clone(&host.missing_cwd.create_ui),
            theme: Rc::clone(&host.missing_cwd.theme),
            presenter: Rc::clone(&host.missing_cwd.presenter),
        },
    };
    let selected = create_session_manager(
        &startup.parsed,
        &dirs,
        startup.app_mode,
        effects,
        presentations,
    );
    Ok(selected.await?.map_continue(|manager| (manager, dirs)))
}

/// Create the runtime over owned startup choices that later replacements reuse.
async fn prepare_runtime(
    parsed: &mut Args,
    manager: SessionManager,
    dirs: &SessionDirs,
    options: MainOptions,
    (host, offline, timings): (&MainHost<'_, '_>, bool, &mut Timings),
) -> Result<Prepared, Box<dyn Error>> {
    let initial = Args {
        messages: std::mem::take(&mut parsed.messages),
        file_args: std::mem::take(&mut parsed.file_args),
        ..Args::default()
    };
    let paths = CliPaths::new(parsed, &dirs.cwd)?;
    let agent_dir = dirs.agent_dir.clone();
    let parsed = Rc::new(std::mem::take(parsed));
    let collator = Rc::new(Collator::new(&|key| std::env::var(key).ok())?);
    let inputs = Rc::new(RuntimeInputs {
        parsed: Rc::clone(&parsed),
        paths,
        extension_factories: options.extension_factories,
        auth: Arc::new(AuthStorage::create(&config::paths::get_auth_path(
            &agent_dir,
        ))),
        local: Rc::clone(&host.local),
        module_loader: Rc::clone(&host.module_loader),
        theme: host.config.theme.theme(),
        collator: Rc::clone(&collator),
        stdout: Rc::clone(&host.package.stdout),
        stderr: Rc::clone(&host.package.stderr),
        color: host.package.styles_enabled,
        offline,
    });
    let factory = runtime_factory(inputs);
    timings.time("createRuntime", host.now);
    let request = CreateAgentSessionRuntimeOptions {
        cwd: manager.get_cwd().to_owned(),
        agent_dir,
        session_manager: Rc::new(RefCell::new(manager)),
        session_start_event: None,
    };
    let runtime = create_agent_session_runtime(factory, request).await?;
    Ok(Prepared {
        runtime: Rc::new(runtime),
        parsed,
        initial,
        collator,
    })
}

/// Print help with the loaded extension flags, or list models; report whether either ran.
fn show_metadata(prepared: &Prepared, host: &MainHost<'_, '_>) -> Result<bool, Box<dyn Error>> {
    let styles = host.package.styles_enabled;
    if prepared.parsed.help == Some(true) {
        let extensions = prepared.runtime.services().resource_loader.get_extensions();
        let flags: Vec<_> = extensions
            .extensions
            .iter()
            .flat_map(|extension| {
                let extension = extension
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                extension.flags.values().cloned().collect::<Vec<_>>()
            })
            .collect();
        print_help(&flags, &mut *host.package.stdout.borrow_mut(), styles)?;
        return Ok(true);
    }
    let Some(listing) = &prepared.parsed.list_models else {
        return Ok(false);
    };
    let search_pattern = match listing {
        ListModels::All => None,
        ListModels::Search(pattern) => Some(pattern.as_str()),
    };
    let options = ListModelsOptions {
        search_pattern,
        docs_path: &config::get_docs_path,
        collator: &prepared.collator,
        styles_enabled: styles,
    };
    let registry = &prepared.runtime.services().model_registry;
    let mut stdout = SharedWriter(Rc::clone(&host.package.stdout));
    let mut stderr = SharedWriter(Rc::clone(&host.package.stderr));
    list_models(registry, options, &mut stdout, &mut stderr)?;
    Ok(true)
}

/// Read input, prepare the first prompt and theme, report diagnostics, then run the mode.
async fn finish(
    mut prepared: Prepared,
    host: MainHost<'_, '_>,
    (mut app_mode, migrations): (AppMode, MigrationResult),
    timings: &mut Timings,
) -> Result<i32, Box<dyn Error>> {
    let mut stdin = None;
    if app_mode != AppMode::Rpc {
        stdin = read_piped_stdin(&mut *host.stdin, (host.stdin_is_tty)()).await?;
        if stdin.is_some() && app_mode.is_interactive() {
            app_mode = AppMode::Print;
        }
    }
    timings.time("readPipedStdin", host.now);
    let services = prepared.runtime.services();
    let auto_resize = services.settings_manager.borrow().get_image_auto_resize();
    let first = prepare_initial_message(
        &mut prepared.initial,
        auto_resize,
        stdin.as_deref(),
        host.files,
    )?;
    timings.time("prepareInitialMessage", host.now);
    let theme = services.settings_manager.borrow().get_theme();
    let watcher = app_mode
        .is_interactive()
        .then(|| Rc::clone(&host.config.watcher));
    host.config.theme.init_theme(theme.as_deref(), watcher)?;
    timings.time("initTheme", host.now);
    if app_mode.is_interactive() {
        let mut output = migration_output(&host);
        let warnings = &migrations.deprecation_warnings;
        show_deprecation_warnings(warnings, &mut output, wait_for_input()).await?;
    }
    let scoped_models = prepared.runtime.session().scoped_models();
    timings.time("resolveModelScope", host.now);
    if report_runtime(&prepared.runtime, app_mode, &host, timings)? {
        return Ok(1);
    }
    let request = ModeDispatch {
        app_mode,
        startup_benchmark: std::env::var("MAESTRO_STARTUP_BENCHMARK").ok(),
        interactive_options: interactive_options(&mut prepared, first, &host, migrations),
        scoped_models,
    };
    let mut stdout = SharedWriter(Rc::clone(&host.package.stdout));
    let mut stderr = SharedWriter(Rc::clone(&host.package.stderr));
    let mut output = DispatchOutput {
        stdout: &mut stdout,
        stderr: &mut stderr,
        timings,
        now: host.now,
        styles_enabled: host.package.styles_enabled,
    };
    dispatch_mode(
        prepared.runtime,
        request,
        &host.modes,
        &host.config.theme,
        &mut output,
    )
    .await
}

/// Assemble the presentation inputs after startup has prepared the first prompt.
fn interactive_options(
    prepared: &mut Prepared,
    first: InitialMessageResult,
    host: &MainHost<'_, '_>,
    migrations: MigrationResult,
) -> InteractiveModeOptions {
    InteractiveModeOptions {
        verbose: prepared.parsed.verbose == Some(true),
        migrated_providers: migrations.migrated_auth_providers,
        model_fallback_message: prepared.runtime.model_fallback_message(),
        initial_message: first.initial_message,
        initial_images: first.initial_images,
        initial_messages: std::mem::take(&mut prepared.initial.messages).into(),
        service_origin: Some(host.package.service_origin.to_owned())
            .filter(|origin| !origin.is_empty()),
        fetch: host.package.version_options.fetch.clone(),
    }
}

/// Report runtime errors, sample session creation, then check the no-model fallback.
fn report_runtime(
    runtime: &AgentSessionRuntime,
    app_mode: AppMode,
    host: &MainHost<'_, '_>,
    timings: &mut Timings,
) -> Result<bool, Box<dyn Error>> {
    let diagnostics = runtime.diagnostics();
    let styles = host.package.styles_enabled;
    report_diagnostics(&diagnostics, &mut *host.package.stderr.borrow_mut(), styles)?;
    if diagnostics.iter().any(|diagnostic| {
        diagnostic.r#type == maestro_app::AgentSessionRuntimeDiagnosticType::Error
    }) {
        return Ok(true);
    }
    timings.time("createAgentSession", host.now);
    let fallback = runtime.model_fallback_message();
    if app_mode.is_interactive() || fallback.is_none() {
        return Ok(false);
    }
    let docs = config::get_docs_path()?;
    let message = maestro_app::format_no_models_available_message(&docs);
    if fallback.as_deref() != Some(message.as_str()) {
        return Ok(false);
    }
    line(&host.package.stderr, &Theme::red(&message, styles))?;
    Ok(true)
}

/// Wait for one key after the conversion notices on the native terminal.
#[cfg(any(unix, windows))]
fn wait_for_input() -> impl Future<Output = io::Result<()>> {
    maestro_tui_crossterm::terminal::wait_for_input()
}

/// Continue at once where no native terminal exists to read a key from.
#[cfg(not(any(unix, windows)))]
fn wait_for_input() -> impl Future<Output = io::Result<()>> {
    std::future::ready(Ok(()))
}
