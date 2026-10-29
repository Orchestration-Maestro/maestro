//! Server mode: binds the API socket, announces it and serves it beside the daemon loop
//! until the daemon stops.
use super::server::Server;
use maestro_app::SettingsManager;
use maestro_app::daemon::pane::launch_env::STARTUP_CWD_ENV;
use maestro_app::daemon::{app::App, logging, paths::DaemonPaths, platform};
use maestro_app::rpc_mode::DaemonWorkspaceOperations;
use maestro_app::rpc_types::workspace_schema::ServerCapabilities;
use std::path::{Path, PathBuf};
use std::{future::Future, io, rc::Rc};

/// The log role of the server.
const LOG_ROLE: &str = "daemon";

/// Serves the daemon's API socket while `app` runs its loop, until `shutdown` resolves or a
/// client stops the daemon; then removes the socket unless another file replaced it.
///
/// Logs to the daemon log of `paths`, filtered by a valid `MAESTRO_LOG` directive. Once the
/// socket is bound, opens the first workspace in the directory `MAESTRO_STARTUP_CWD` names
/// when the daemon has none, then writes the ready message to standard error. A
/// configuration reload reads through `settings`. Must run inside a
/// [`tokio::task::LocalSet`].
///
/// # Errors
/// Returns the socket preparation or binding failure: address-in-use while another daemon
/// serves the socket, already-exists for a file that is not a socket.
pub async fn run_server<C: FnMut() -> u128>(
    app: &mut App<C>,
    settings: &mut SettingsManager,
    paths: &DaemonPaths,
    shutdown: impl Future<Output = ()>,
) -> io::Result<()> {
    let filter = std::env::var("MAESTRO_LOG").ok();
    logging::init_file_logging(paths, LOG_ROLE, filter.as_deref());
    let path = PathBuf::from(paths.api_socket_path());
    let server = Server::bind(&path).inspect_err(|error| {
        if error.kind() == io::ErrorKind::AddrInUse {
            eprintln!("error: maestro server is already running");
            eprintln!("api socket: {}", path.display());
        }
    })?;
    app.seed_startup_workspace(std::env::var_os(STARTUP_CWD_ENV).map(PathBuf::from));
    tracing::info!(api_socket = %path.display(), "maestro server started");
    announce(&path, paths);
    let (sender, mut requests) = tokio::sync::mpsc::unbounded_channel();
    let (stop, mut stopped) = tokio::sync::watch::channel(false);
    let operations = Rc::new(DaemonWorkspaceOperations::new(
        sender,
        Some(capabilities()),
        stop,
    ));
    let shutdown = async move {
        tokio::select! {
            () = shutdown => {}
            _ = stopped.wait_for(|stopped| *stopped) => {}
        }
    };
    tokio::select! {
        () = async {
            app.run(paths, &mut requests, settings, shutdown).await;
            operations.wait_for_stop_response().await;
        } => {}
        never = server.serve(operations.clone()) => match never {},
    }
    drop(server);
    logging::shutdown(LOG_ROLE);
    Ok(())
}

/// Writes the ready message: the server runs, with its bound API socket and its log.
fn announce(api_socket: &Path, paths: &DaemonPaths) {
    eprintln!("maestro server running; you can use any maestro CLI command in another terminal.");
    eprintln!("api socket: {}", api_socket.display());
    eprintln!("logs: {}", paths.log_path(LOG_ROLE));
}

/// What a ping reports the server can do; live handoff stays off until the server performs
/// one.
fn capabilities() -> ServerCapabilities {
    ServerCapabilities {
        live_handoff: false,
        detached_server_daemon: platform::current_process_is_detached_server_daemon(),
    }
}
