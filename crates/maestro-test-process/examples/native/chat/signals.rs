//! Programs that wait for a stop signal, a termination or the test's input.

use std::io::{self, Read};
use std::os::unix::net::UnixStream;
use std::sync::mpsc;

use maestro_test_process::child::{leave, write_stdout};
use signal_hook::consts::SIGTSTP;

/// Wait until a signal ends the process; no data ever arrives on the connection read.
pub(super) fn pause() -> io::Result<()> {
    let (mut reader, _writer) = UnixStream::pair()?;
    reader.read_exact(&mut [0])
}

/// One thing the group member waits for.
enum Event {
    /// A stop signal reached the process.
    Stopped,
    /// A line of input arrived or the input ended.
    Input,
}

/// Report `ready` once the stop signal is handled, then report `TSTP` when it arrives, or end
/// silently when input arrives first.
pub(super) fn group_member() -> io::Result<()> {
    let (mut notified, writer) = UnixStream::pair()?;
    let _registration = signal_hook::low_level::pipe::register(SIGTSTP, writer)?;
    write_stdout(b"ready\n")?;
    let (events, outcomes) = mpsc::channel();
    let stopped = events.clone();
    std::thread::spawn(move || {
        if notified.read_exact(&mut [0]).is_ok() {
            let _ = stopped.send(Event::Stopped);
        }
    });
    std::thread::spawn(move || {
        let mut line = [0];
        let _ = std::io::stdin().read(&mut line);
        let _ = events.send(Event::Input);
    });
    if let Ok(Event::Stopped) = outcomes.recv() {
        write_stdout(b"TSTP\n")?;
    }
    leave(0)
}
