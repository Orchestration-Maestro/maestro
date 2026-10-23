//! Descriptor registration shared by input and output, with regular-file fallback.

use std::cell::Cell;
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
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
    /// A descriptor the reactor can wait on: a terminal, pipe or socket.
    Reactor(AsyncFd<OwnedFd>),
    /// A descriptor the reactor refuses with a permission error, such as a regular file or
    /// `/dev/null`, which a read does not wait on.
    File(File),
}

impl Source {
    /// Duplicates a descriptor, saving output flags before enabling nonblocking writes.
    pub(super) fn open(fd: impl AsFd, interest: Interest) -> io::Result<Self> {
        let duplicate = rustix::io::fcntl_dupfd_cloexec(fd, 3)?;
        let flags = interest
            .is_writable()
            .then(|| fcntl_getfl(&duplicate))
            .transpose()?;
        // The advisory covers a descriptor the registration does not own; this one owns it.
        #[expect(
            deprecated,
            reason = "owns its descriptor; the advisory concerns unowned ones"
        )]
        let registration = AsyncFd::try_with_interest(duplicate, interest);
        let kind = match registration {
            Ok(descriptor) => Kind::Reactor(descriptor),
            Err(refused) => match refused.into_parts() {
                (descriptor, error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    Kind::File(File::from(descriptor))
                }
                (_, error) => return Err(error),
            },
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
}
