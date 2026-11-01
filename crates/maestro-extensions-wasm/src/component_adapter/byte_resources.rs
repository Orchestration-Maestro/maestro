//! Raw resources keep their original host handles across callbacks.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::Imports;
use crate::{
    ExtensionFuture, ExtensionResult,
    child_process::{ChildProcess, ExitStatus},
    streams::{Readable, Writable},
};
use std::{cell::OnceCell, rc::Rc};

/// An independently owned host resource behind an author trait.
pub(super) struct Resource<I: Imports, R> {
    /// Original imports, not a process-global host lookup.
    pub(super) imports: I,
    /// Original consuming resource.
    pub(super) handle: R,
}
impl<I: Imports, R> Resource<I, R> {
    /// Retains the selected resource and its import capability.
    pub(super) fn new(imports: &I, handle: R) -> Self {
        Self {
            imports: imports.clone(),
            handle,
        }
    }
}
impl<I: Imports> Readable for Resource<I, I::Readable> {
    fn read(&self) -> ExtensionFuture<'_, Option<Vec<u8>>> {
        self.imports.readable_read(&self.handle)
    }
    fn close(&self) -> ExtensionResult<()> {
        self.imports.readable_close(&self.handle)
    }
}
impl<I: Imports> Writable for Resource<I, I::Writable> {
    fn write<'a>(&'a self, bytes: &'a [u8]) -> ExtensionFuture<'a, ()> {
        self.imports.writable_write(&self.handle, bytes)
    }
    fn end(&self) -> ExtensionFuture<'_, ()> {
        self.imports.writable_end(&self.handle)
    }
}
/// A child retains each selected pipe once, so getters preserve author identity.
pub(super) struct Child<I: Imports> {
    /// The original child control resource.
    resource: Resource<I, I::Child>,
    /// Original input alias, including an absent descriptor.
    stdin: OnceCell<Option<Rc<dyn Writable>>>,
    /// Original output alias, including an absent descriptor.
    stdout: OnceCell<Option<Rc<dyn Readable>>>,
    /// Original error alias, including an absent descriptor.
    stderr: OnceCell<Option<Rc<dyn Readable>>>,
}
impl<I: Imports> Child<I> {
    /// Retains immutable descriptor selections without resetting any consuming handle.
    pub(super) fn new(imports: &I, handle: I::Child) -> Self {
        Self {
            resource: Resource::new(imports, handle),
            stdin: OnceCell::new(),
            stdout: OnceCell::new(),
            stderr: OnceCell::new(),
        }
    }
}
impl<I: Imports> ChildProcess for Child<I> {
    fn pid(&self) -> Option<u32> {
        self.resource.imports.child_pid(&self.resource.handle)
    }
    fn stdin(&self) -> Option<Rc<dyn Writable>> {
        self.stdin
            .get_or_init(|| {
                self.resource
                    .imports
                    .child_stdin(&self.resource.handle)
                    .map(|handle| {
                        Rc::new(Resource::new(&self.resource.imports, handle)) as Rc<dyn Writable>
                    })
            })
            .clone()
    }
    fn stdout(&self) -> Option<Rc<dyn Readable>> {
        self.stdout
            .get_or_init(|| {
                self.resource
                    .imports
                    .child_stdout(&self.resource.handle)
                    .map(|handle| {
                        Rc::new(Resource::new(&self.resource.imports, handle)) as Rc<dyn Readable>
                    })
            })
            .clone()
    }
    fn stderr(&self) -> Option<Rc<dyn Readable>> {
        self.stderr
            .get_or_init(|| {
                self.resource
                    .imports
                    .child_stderr(&self.resource.handle)
                    .map(|handle| {
                        Rc::new(Resource::new(&self.resource.imports, handle)) as Rc<dyn Readable>
                    })
            })
            .clone()
    }
    fn wait(&self) -> ExtensionFuture<'_, ExitStatus> {
        self.resource.imports.child_wait(&self.resource.handle)
    }
    fn kill(&self, signal: Option<String>) -> ExtensionResult<bool> {
        self.resource
            .imports
            .child_kill(&self.resource.handle, signal)
    }
}
