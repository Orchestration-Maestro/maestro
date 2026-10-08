//! Lexical operations on authored path strings.
//!
//! An authored path is the text a user, setting or file wrote. These functions
//! join, normalize, resolve and split that text without touching the file
//! system, the process working directory or the environment, and follow the
//! JavaScript runtime's `path` module for both of its flavors. Convert an
//! authored path to [`std::path::PathBuf`] only at a file-system call.
//!
//! The [`posix`] and [`win32`] modules behave the same on every platform. The
//! crate root re-exports the flavor of the compile target; the POSIX-only
//! [`posix::parse`] is also re-exported on targets other than Windows.

mod path;

pub use path::{Cwd, posix, win32};

#[cfg(not(windows))]
pub use path::posix::{
    ParsedPath, SEP, basename, dirname, is_absolute, join, normalize, parse, relative, resolve,
};
#[cfg(windows)]
pub use path::win32::{SEP, basename, dirname, is_absolute, join, normalize, relative, resolve};
