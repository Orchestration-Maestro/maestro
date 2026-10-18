//! Owned pane terminals and live process facts on Linux.

pub mod foreground;

mod backend;
mod fd;
pub use backend::{setup_and_exec, spawn};

use std::{ffi::OsString, io, os::fd::OwnedFd, path::PathBuf};

/// An owned terminal master.
#[derive(Debug)]
pub struct Pty {
    /// Owned terminal descriptor.
    pub master_fd: OwnedFd,
}

/// Terminal cells and per-cell pixel dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PtySize {
    /// Number of rows.
    pub rows: u16,
    /// Number of columns.
    pub cols: u16,
    /// Width of one cell in pixels.
    pub cell_width_px: u32,
    /// Height of one cell in pixels.
    pub cell_height_px: u32,
}

/// Owned native launch operands passed through the setup-entry adapter.
#[derive(Debug)]
pub struct PtyCommand {
    /// Native executable operand.
    pub program: OsString,
    /// Ordered native arguments.
    pub args: Vec<OsString>,
    /// Working directory selected before launch.
    pub cwd: Option<PathBuf>,
    /// Explicit inherited or replacement environment policy.
    pub environment: PtyEnvironment,
}

/// Environment construction for a pane child.
#[derive(Debug)]
pub enum PtyEnvironment {
    /// Ordered edits to the inherited environment; None removes a variable.
    Inherit(Vec<(OsString, Option<OsString>)>),
    /// Start empty and apply these values, including an empty replacement.
    Replace(Vec<(OsString, OsString)>),
}

/// Standard child owner, separate from the terminal master.
pub type PtyChild = std::process::Child;

impl Pty {
    /// Applies cell dimensions with saturated total pixel dimensions.
    ///
    /// # Errors
    /// Returns the native window-size error.
    pub fn resize(&self, size: PtySize) -> io::Result<()> {
        fd::resize_pty_fd(&self.master_fd, size)
    }

    /// Reads the terminal's current foreground process group.
    #[must_use]
    pub fn foreground_process_group(&self) -> Option<u32> {
        foreground::foreground_process_group_id_for_tty_fd(&self.master_fd)
    }
}

pub mod actor;
