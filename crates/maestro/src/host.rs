//! Native package, stream and export effects supplied to command-line startup.
use maestro_app::configuration::{
    config::{self, SelfUpdateContext, installation::Installation},
    version_check::VersionCheckOptions,
};
use maestro_cli::{MainOptions, packages::PackageCommandContext, startup::main::MainHost};
use maestro_theme::{ThemeState, supports_color};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    io::{self, IsTerminal, Write},
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::task::LocalSet;

/// Ordinary output follows the application's existing stdout reservation.
struct OrdinaryOutput;
impl Write for OrdinaryOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if maestro_app::output_guard::is_stdout_taken_over() {
            io::stderr().write(bytes)
        } else {
            io::stdout().write(bytes)
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        if maestro_app::output_guard::is_stdout_taken_over() {
            io::stderr().flush()
        } else {
            io::stdout().flush()
        }
    }
}

/// Process-lifetime presentation and export owners, shared by every runtime replacement.
fn themes() -> io::Result<&'static Rc<ThemeState>> {
    Ok(Box::leak(Box::new(Rc::new(ThemeState::new(
        config::get_theme_directories()?,
    )))))
}

/// Prepare the native effects without duplicating command-line policy.
pub async fn run(args: Vec<String>, local: Rc<LocalSet>) -> Result<i32, Box<dyn Error>> {
    let themes = themes()?;
    let installation = Installation::current()?;
    let package_dir = config::get_package_dir()?;
    let updates = config::NativeSelfUpdateOperations;
    let update = SelfUpdateContext {
        installation: &installation,
        package_dir: &package_dir,
        operations: &updates,
    };
    let offline = Rc::new(Cell::new(None));
    let status = Cell::new(None);
    let create = |options| Ok(package_manager(options, &local, offline.get()));
    let select = |settings: &maestro_app::Settings, name: &str| {
        config::get_self_update_command_from_settings(&update, settings, name)
    };
    let instruction = |settings: &maestro_app::Settings, name: &str| {
        config::get_self_update_unavailable_instruction_from_settings(&update, settings, name)
    };
    let version = VersionCheckOptions {
        fetch: Some(maestro_app::default_fetch()),
        ..Default::default()
    };
    let load = |cwd: &str, agent: &str| Ok(settings(cwd, agent));
    let package = PackageCommandContext {
        cwd: &maestro_app::current_directory,
        agent_dir: &config::get_agent_dir,
        load_settings: &load,
        create_package_manager: &create,
        select_self_update_command: &select,
        self_update_unavailable_instruction: &instruction,
        self_update_operations: &updates,
        version_options: &version,
        service_origin: config::SERVICE_ORIGIN,
        stdout: Rc::new(RefCell::new(OrdinaryOutput)),
        stderr: Rc::new(RefCell::new(io::stderr())),
        exit_code: &status,
        entrypoint: Some(&installation.executable),
        styles_enabled: supports_color(),
    };
    let effects = NativeStartup {
        local: Rc::clone(&local),
        themes,
        offline: Rc::clone(&offline),
        export: exports(themes, &installation, &package_dir),
        terminal: Rc::new(RefCell::new(maestro_tui_crossterm::ProcessTerminal::new(
            Rc::clone(&local),
        ))),
    };
    startup(args, package, effects).await
}

/// The native presentation handles one startup retains together.
struct NativeStartup {
    /// Caller-driven local work.
    local: Rc<LocalSet>,
    /// Shared process-lifetime theme owner.
    themes: &'static Rc<ThemeState>,
    /// Offline choice sampled before package dispatch.
    offline: Rc<Cell<Option<bool>>>,
    /// Native supplied-bundle export adapter.
    export: Rc<dyn maestro_app::ExportOperations>,
    /// The unstarted native terminal shared by component output and interactive frames.
    terminal: Rc<RefCell<maestro_tui_crossterm::ProcessTerminal>>,
}

/// Bind the native presentations and input to the existing startup entry.
async fn startup(
    args: Vec<String>,
    package: PackageCommandContext<'_>,
    effects: NativeStartup,
) -> Result<i32, Box<dyn Error>> {
    let NativeStartup {
        local,
        themes,
        offline,
        export,
        terminal,
    } = effects;
    let now = || {
        (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64()
            * 1000.0)
            .floor()
    };
    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin());
    let files =
        maestro_cli::input::file_processor::NativeFileArgumentOperations::new(supports_color());
    let host = MainHost {
        package,
        apply_startup: Rc::new(move |value| offline.set(Some(value))),
        config: crate::presentations::config(Rc::clone(&local), Rc::clone(themes)),
        sessions: crate::presentations::sessions(Rc::clone(&local), Rc::clone(themes)),
        missing_cwd: crate::presentations::missing_cwd(Rc::clone(&local), Rc::clone(themes)),
        local: Rc::clone(&local),
        module_loader: module_loader(themes.theme(), &local, &terminal.borrow()),
        modes: crate::modes::launchers(
            Rc::clone(&local),
            themes,
            Rc::clone(&export),
            terminal.clone(),
        ),
        stdin: &mut stdin,
        stdin_is_tty: &|| io::stdin().is_terminal(),
        now: &now,
        files: &files,
        export_from_file: Rc::new(move |source, destination| {
            maestro_app::export_from_file(
                source,
                Some(maestro_app::ExportOptions {
                    output_path: destination,
                    ..Default::default()
                }),
                &*export,
            )
            .map_err(Into::into)
        }),
    };
    maestro_cli::main(args, MainOptions::default(), host).await
}

/// Loads each component path with the application's native effects; the host widgets are
/// built once, when the first loads.
fn module_loader(
    theme: maestro_theme::LiveTheme,
    local: &Rc<LocalSet>,
    terminal: &maestro_tui_crossterm::ProcessTerminal,
) -> maestro_app::ModuleLoader<maestro_app::Local> {
    let stdout = terminal.standard_output_writer();
    let stdout_is_terminal = io::stdout().is_terminal();
    let widgets = Rc::new(std::cell::OnceCell::new());
    let io = maestro_app::extension_native_io::extension_native_io(local);
    Rc::new(move |path: &str| {
        let factory = extension_widgets(&widgets, &theme).and_then(|widgets| {
            Ok(Some(maestro_extensions_wasmtime::artifact_factory_with_io(
                path.to_owned(),
                widgets,
                maestro_extensions_wasmtime::Installation {
                    version: config::VERSION.into(),
                    agent_dir: config::get_agent_dir()
                        .map_err(|error| maestro_app::EventError::from(error.to_string()))?,
                },
                Rc::clone(&io),
                Some(maestro_extensions_wasmtime::Stdout::new(
                    Box::new(stdout.clone()),
                    stdout_is_terminal,
                )),
            )))
        });
        Box::pin(std::future::ready(factory))
    })
}

/// The host widgets of every component, with the live theme, the code highlighting and the
/// terminal's capabilities.
fn extension_widgets(
    cell: &std::cell::OnceCell<Rc<dyn maestro_app::ExtensionWidgets>>,
    theme: &maestro_theme::LiveTheme,
) -> Result<Rc<dyn maestro_app::ExtensionWidgets>, maestro_app::EventError> {
    if let Some(widgets) = cell.get() {
        return Ok(Rc::clone(widgets));
    }
    let highlighter = Rc::new(maestro_theme::SyntectHighlighter::new()?);
    let terminal = maestro_tui::TerminalImage::default();
    let host = maestro_chat::ExtensionWidgetHost::new(theme.clone(), highlighter, terminal);
    Ok(Rc::clone(cell.get_or_init(|| Rc::new(host))))
}

/// Retain the central export bundle path for this executable's lifetime.
fn exports(
    themes: &'static Rc<ThemeState>,
    installation: &Installation,
    package_dir: &str,
) -> Rc<dyn maestro_app::ExportOperations> {
    let path = config::paths::get_export_template_dir(package_dir, installation.layout, |path| {
        std::path::Path::new(path).exists()
    });
    Rc::new(maestro_app::NativeExportOperations {
        app_name: config::APP_NAME,
        template_dir: Box::leak(path.into_boxed_str()),
        theme: themes.as_ref(),
    })
}

/// Construct the package manager over its supplied settings and local task owner.
fn package_manager(
    options: maestro_app::PackageManagerOptions,
    local: &Rc<LocalSet>,
    offline: Option<bool>,
) -> Rc<dyn maestro_app::PackageManager> {
    Rc::new(
        maestro_app::DefaultPackageManager::new(
            options,
            maestro_app::NativePackageOperations::new(
                Rc::new(maestro_app::output_guard::is_stdout_taken_over),
                local,
            ),
        )
        .with_offline(offline),
    )
}

/// Read accepted settings through the existing application owner.
fn settings(cwd: &str, agent: &str) -> Rc<RefCell<maestro_app::SettingsManager>> {
    Rc::new(RefCell::new(maestro_app::SettingsManager::create(
        cwd,
        agent,
        config::CONFIG_DIR_NAME,
    )))
}
