//! Safe owned terminal descriptor operations.
use super::PtySize;
use std::{
    io,
    os::fd::{AsFd, OwnedFd},
};

/// Duplicates a descriptor with close-on-exec set atomically.
pub(crate) fn duplicate(fd: impl AsFd) -> io::Result<OwnedFd> {
    rustix::io::fcntl_dupfd_cloexec(fd, 3).map_err(Into::into)
}

/// Enables nonblocking I/O without discarding existing flags.
pub(crate) fn set_nonblocking(fd: impl AsFd) -> io::Result<()> {
    let flags = rustix::fs::fcntl_getfl(&fd)?;
    rustix::fs::fcntl_setfl(fd, flags | rustix::fs::OFlags::NONBLOCK)?;
    Ok(())
}

/// Applies rows, columns and saturated total pixel dimensions.
pub(crate) fn resize_pty_fd(fd: impl AsFd, size: PtySize) -> io::Result<()> {
    let pixels = |cells: u16, per_cell: u32| {
        u16::try_from(u32::from(cells).saturating_mul(per_cell)).unwrap_or(u16::MAX)
    };
    rustix::termios::tcsetwinsize(
        fd,
        rustix::termios::Winsize {
            ws_row: size.rows,
            ws_col: size.cols,
            ws_xpixel: pixels(size.cols, size.cell_width_px),
            ws_ypixel: pixels(size.rows, size.cell_height_px),
        },
    )?;
    Ok(())
}
