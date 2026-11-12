//! Helpers that the layout apply tests share between the integration program and the unit tests.

use std::{
    io::{self, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::Path,
};

/// Run the layout operation with a checked relay, returning as soon as either fails.
///
/// # Errors
/// Returns the relay failure.
pub async fn relay_hangup(
    socket: &Path,
    body: impl std::future::Future<Output = ()>,
) -> io::Result<()> {
    let listener = UnixListener::bind(socket)?;
    let (report, result) = tokio::sync::oneshot::channel();
    let relay = std::thread::spawn(move || {
        // A failed body may drop its receiver before the relay ends.
        let _ = report.send(release_after_hangup(&listener));
    });
    tokio::try_join!(
        async {
            result.await.map_err(io::Error::other)??;
            relay
                .join()
                .map_err(|_| io::Error::other("hangup relay panicked"))
        },
        async {
            body.await;
            Ok(())
        }
    )?;
    Ok(())
}

/// Release the one waiting leaf after the one holder reports its hangup, in either arrival order.
///
/// # Errors
/// Returns the failure of the relay socket or an unknown announcement.
pub fn release_after_hangup(listener: &UnixListener) -> io::Result<()> {
    let mut waiting = None;
    let mut hung_up = false;
    for stream in listener.incoming() {
        let mut stream = stream?;
        let mut kind = [0];
        stream.read_exact(&mut kind)?;
        match kind {
            [b'h'] => hung_up = true,
            [b'w'] => waiting = Some(stream),
            _ => return Err(io::Error::other("unknown relay announcement")),
        }
        if hung_up && let Some(leaf) = waiting.take() {
            release(leaf);
            return Ok(());
        }
    }
    Err(io::Error::other("hangup relay closed before release"))
}

/// Sends the release byte; a leaf that already ended needs none.
pub fn release(mut leaf: UnixStream) {
    let _ = leaf.write_all(b"x");
}
