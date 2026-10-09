//! Descriptor and state helpers for the lifecycle tests that the output tests do not need.
#![cfg(test)]

mod utf8_streams;

use std::fs::File;
use std::io::{Seek, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{OFlags, fcntl_getfl};
use rustix::io::read;
use rustix::process::{Resource, Rlimit, Signal, getpid, getrlimit, kill_process, setrlimit};
use rustix::stdio::{dup2_stdin, stdin};
use rustix::termios::{Termios, tcgetattr};
use tokio::runtime::Builder;

use crate::support::{Feed, Inputs, Resizes, Rig, pty, set_size, until};

pub use utf8_streams::UTF8_STREAMS;

impl Rig {
    /// A rig on real time.
    pub fn real() -> Self {
        Self::on(Builder::new_current_thread().enable_all())
    }
}

/// Installs a pseudo-terminal as standard input and returns its controlling end.
pub fn pty_input() -> Feed {
    let (master, slave) = pty();
    dup2_stdin(&slave).expect("the slave becomes standard input");
    Feed(master)
}

/// Installs a socket as standard input and returns its peer.
pub fn socket_input() -> Feed {
    let (near, far) = UnixStream::pair().expect("a socket pair opens");
    dup2_stdin(&near).expect("the socket becomes standard input");
    Feed(far.into())
}

/// Installs a directory as standard input, which the reactor refuses and reading fails on.
pub fn directory_input() {
    let directory = File::open("/").expect("the root directory opens");
    dup2_stdin(&directory).expect("the directory becomes standard input");
}

/// Installs a regular file holding `content` as standard input.
pub fn file_input(content: &[u8]) {
    let path = std::env::temp_dir().join(format!("maestro-terminal-{}", std::process::id()));
    let mut file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .expect("the file is created");
    std::fs::remove_file(path).expect("the file is unlinked");
    file.write_all(content).expect("the file takes its content");
    file.rewind().expect("the file rewinds");
    dup2_stdin(&file).expect("the file becomes standard input");
}

impl Feed {
    /// Closes the writing end, which ends the input.
    pub fn close(self) {
        drop(self);
    }

    /// Sets the window size of the pseudo-terminal this feed controls.
    pub fn resize(&self, columns: u16, rows: u16) {
        set_size(&self.0, columns, rows);
    }
}

impl Inputs {
    /// The chunks delivered so far.
    pub fn all(&self) -> Vec<String> {
        self.0.borrow().clone()
    }

    /// How many chunks were delivered so far.
    pub fn count(&self) -> usize {
        self.0.borrow().len()
    }

    /// Waits until at least `count` chunks were delivered.
    pub async fn delivered(&self, count: usize) {
        until(|| self.count() >= count).await;
    }
}

impl Resizes {
    /// How many notices were delivered so far.
    pub fn count(&self) -> usize {
        self.0.get()
    }

    /// Waits until at least `count` notices were delivered.
    pub async fn delivered(&self, count: usize) {
        until(|| self.count() >= count).await;
    }
}

/// Makes opening one more descriptor fail with `EMFILE` until it is dropped.
pub struct DescriptorLimit(Rlimit);

impl DescriptorLimit {
    /// Lowers the soft limit to the number of the lowest free descriptor.
    pub fn reached() -> Self {
        let original = getrlimit(Resource::Nofile);
        let probe = rustix::io::dup(stdin()).expect("a probe descriptor opens");
        let lowest_free = u64::try_from(probe.as_raw_fd()).expect("a descriptor is not negative");
        drop(probe);
        let lowered = Rlimit {
            current: Some(lowest_free),
            maximum: original.maximum,
        };
        setrlimit(Resource::Nofile, lowered).expect("the limit is lowered");
        Self(original)
    }
}

impl Drop for DescriptorLimit {
    fn drop(&mut self) {
        setrlimit(Resource::Nofile, self.0).expect("the limit is restored");
    }
}

/// Whether standard input holds bytes nobody has read.
fn has_unread_input() -> bool {
    let stdin = stdin();
    let mut fds = [PollFd::new(&stdin, PollFlags::IN)];
    let none = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    poll(&mut fds, Some(&none)).expect("poll") > 0 && fds[0].revents().contains(PollFlags::IN)
}

/// The bytes standard input holds unread, without waiting for more.
pub fn unread_input() -> Vec<u8> {
    let mut bytes = vec![0; 256];
    if !has_unread_input() {
        return Vec::new();
    }
    let count = read(stdin(), &mut bytes).expect("input reads");
    bytes.truncate(count);
    bytes
}

/// Waits until the terminal has read everything the scenario sent to standard input. For a
/// descriptor the reactor reads, framing and delivery follow the read in the same step of the
/// single-threaded runtime, so they have happened when this returns.
pub async fn consumed() {
    until(|| !has_unread_input()).await;
}

/// Sends the window-change signal to this process.
pub fn window_change() {
    kill_process(getpid(), Signal::WINCH).expect("the signal is sent");
}

/// The attributes of standard input as text, for exact before-and-after comparison.
pub fn stdin_attributes() -> String {
    format!("{:?}", attributes(stdin()))
}

/// The terminal attributes of `fd`.
pub fn attributes(fd: impl std::os::fd::AsFd) -> Termios {
    tcgetattr(fd).expect("a terminal")
}

/// The status flags of standard input.
pub fn stdin_flags() -> OFlags {
    fcntl_getfl(stdin()).expect("flags read")
}

/// How many descriptors this process holds open.
pub fn open_descriptors() -> usize {
    std::fs::read_dir("/dev/fd")
        .expect("descriptors list")
        .count()
}
