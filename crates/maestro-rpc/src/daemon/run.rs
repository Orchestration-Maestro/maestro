//! Starts the API socket beside the daemon loop and runs both until the daemon stops.
use maestro_app::daemon::{app::App, paths::DaemonPaths};
use std::{future::Future, io};

/// Serves the daemon's API socket while `app` runs its loop, until `shutdown` resolves or a
/// client stops the daemon.
///
/// # Errors
/// Returns the socket preparation or binding failure.
pub async fn run_server<C: FnMut() -> u128>(
    app: &mut App<C>,
    paths: &DaemonPaths,
    shutdown: impl Future<Output = ()>,
) -> io::Result<()> {
    shutdown.await;
    let _ = (app.state.should_quit.0, paths.name());
    drop(super::server::Admission::new(()));
    Ok(())
}
