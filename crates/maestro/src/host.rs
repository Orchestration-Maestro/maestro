//! Native package, stream and export effects supplied to command-line startup.
use maestro_app::configuration::{
    config::{self, SelfUpdateContext, installation::Installation},
    version_check::VersionCheckOptions,
};
use maestro_cli::{MainOptions, packages::PackageCommandContext, startup::main::MainHost};
use maestro_theme::ThemeState;
use std::{
    cell::{Cell, RefCell},
    error::Error,
    io::{self, IsTerminal, Write},
    rc::Rc,
    time::Instant,
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
        styles_enabled: styles_enabled(),
    };
    let effects = NativeStartup {
        local: Rc::clone(&local),
        themes,
        offline: Rc::clone(&offline),
        export: exports(themes, &installation, &package_dir),
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
    } = effects;
    let start = Instant::now();
    let now = || start.elapsed().as_secs_f64() * 1000.0;
    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin());
    let files =
        maestro_cli::input::file_processor::NativeFileArgumentOperations::new(styles_enabled());
    let host = MainHost {
        package,
        apply_startup: Rc::new(move |value| offline.set(Some(value))),
        config: crate::presentations::config(Rc::clone(&local), Rc::clone(themes)),
        sessions: crate::presentations::sessions(Rc::clone(&local), Rc::clone(themes)),
        missing_cwd: crate::presentations::missing_cwd(Rc::clone(&local), Rc::clone(themes)),
        local: Rc::clone(&local),
        module_loader: Rc::new(|path: &str| {
            Box::pin(std::future::ready(Ok(Some(
                maestro_extensions_wasmtime::artifact_factory(path.to_owned()),
            ))))
        }),
        modes: crate::modes::launchers(Rc::clone(&local), themes, Rc::clone(&export)),
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

/// Read the existing native decoration policy.
pub fn styles_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none() && io::stdout().is_terminal()
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
