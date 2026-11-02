//! Descriptor registration shared by input and output, with regular-file fallback.

use std::cell::Cell;
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use rustix::fs::{FileType, OFlags, fcntl_getfl, fcntl_setfl, fstat};
use rustix::termios::isatty;
use tokio::io::{Interest, unix::AsyncFd};

/// A registered or synchronous descriptor and its saved output flags.
pub(super) struct Source {
    /// How readiness is obtained.
    kind: Kind,
    /// Original output flags, restored only once.
    flags: Cell<Option<OFlags>>,
}

/// How readiness is obtained for the duplicated descriptor.
enum Kind {
    /// A descriptor the reactor waits on: a terminal, pipe or socket.
    Reactor(AsyncFd<OwnedFd>),
    /// Any other descriptor, such as a regular file, a directory or `/dev/null`, which a read
    /// or write never waits on.
    File(File),
}

/// Whether reads and writes on `fd` can wait: a terminal, pipe or socket. The file type
/// decides it rather than the reactor's answer, which differs by host: one refuses regular
/// files that another accepts, and refuses `/dev/null` with a different error.
fn waits(fd: BorrowedFd<'_>) -> io::Result<bool> {
    Ok(match FileType::from_raw_mode(fstat(fd)?.st_mode) {
        FileType::Fifo | FileType::Socket => true,
        FileType::CharacterDevice => isatty(fd),
        _ => false,
    })
}

impl Source {
    /// Duplicates a descriptor, saving output flags before enabling nonblocking writes.
    pub(super) fn open(fd: impl AsFd, interest: Interest) -> io::Result<Self> {
        let duplicate = rustix::io::fcntl_dupfd_cloexec(fd, 3)?;
        let flags = interest
            .is_writable()
            .then(|| fcntl_getfl(&duplicate))
            .transpose()?;
        let kind = if waits(duplicate.as_fd())? {
            // The advisory covers a descriptor the registration does not own; this one owns it.
            #[expect(
                deprecated,
                reason = "owns its descriptor; the advisory concerns unowned ones"
            )]
            let registration = AsyncFd::with_interest(duplicate, interest);
            Kind::Reactor(registration?)
        } else {
            Kind::File(File::from(duplicate))
        };
        let source = Self {
            kind,
            flags: Cell::new(flags),
        };
        if let Some(flags) = flags {
            fcntl_setfl(&source, flags | OFlags::NONBLOCK)?;
        }
        Ok(source)
    }

    /// Returns the registered descriptor when the reactor accepts it.
    pub(super) fn reactor(&self) -> Option<&AsyncFd<OwnedFd>> {
        match &self.kind {
            Kind::Reactor(fd) => Some(fd),
            Kind::File(_) => None,
        }
    }

    /// Restores the saved output flags once, before releasing the registration.
    pub(super) fn restore(&self) -> io::Result<()> {
        self.flags
            .take()
            .map_or(Ok(()), |flags| fcntl_setfl(self, flags).map_err(Into::into))
    }

    /// Reads the next available bytes; `Ok(0)` is the end of input.
    pub(super) async fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match &mut self.kind {
            Kind::Reactor(descriptor) => loop {
                let mut ready = descriptor.readable().await?;
                let attempt = ready.try_io(|inner| {
                    rustix::io::read(inner.get_ref(), &mut *buffer).map_err(io::Error::from)
                });
                if let Ok(result) = attempt {
                    return result;
                }
            },
            Kind::File(file) => {
                tokio::task::yield_now().await;
                file.read(buffer)
            }
        }
    }
}

impl AsFd for Source {
    fn as_fd(&self) -> BorrowedFd<'_> {
        match &self.kind {
            Kind::Reactor(fd) => fd.get_ref().as_fd(),
            Kind::File(file) => file.as_fd(),
        }
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

#[cfg(test)]
mod tests {
    use super::Source;
    use rustix::io::{FdFlags, fcntl_getfd};
    use tokio::io::Interest;

    #[test]
    fn descriptor_interests_are_close_on_exec() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _entered = runtime.enter();
        let (socket, _peer) = std::os::unix::net::UnixStream::pair().unwrap();
        let file = std::fs::File::open("/dev/null").unwrap();
        for fd in [&socket as &dyn std::os::fd::AsFd, &file] {
            for interest in [Interest::READABLE, Interest::WRITABLE] {
                let source = Source::open(fd, interest).unwrap();
                assert!(fcntl_getfd(&source).unwrap().contains(FdFlags::CLOEXEC));
            }
        }
    }

    /// Only terminals, pipes and sockets wait for readiness; every other descriptor stays
    /// synchronous, whichever ones the host's reactor would also accept.
    #[test]
    fn only_terminals_pipes_and_sockets_wait() {
        use rustix::pty::{OpenptFlags, openpt};
        use std::fs::File;
        use std::os::fd::AsFd;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _entered = runtime.enter();
        let (socket, _peer) = std::os::unix::net::UnixStream::pair().unwrap();
        let (pipe, _writer) = rustix::pipe::pipe().unwrap();
        let terminal = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).unwrap();
        let regular = File::open(std::env::current_exe().unwrap()).unwrap();
        let directory = File::open("/").unwrap();
        let null = File::open("/dev/null").unwrap();
        let random = File::open("/dev/random").unwrap();
        let cases: [(&dyn AsFd, bool); 7] = [
            (&socket, true),
            (&pipe, true),
            (&terminal, true),
            (&regular, false),
            (&directory, false),
            (&null, false),
            (&random, false),
        ];
        for (index, (fd, waits)) in cases.into_iter().enumerate() {
            let source = Source::open(fd, Interest::READABLE).unwrap();
            assert_eq!(source.reactor().is_some(), waits, "descriptor {index}");
        }
    }
}
