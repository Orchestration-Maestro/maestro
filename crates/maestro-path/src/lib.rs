//! Lexical operations on authored path strings.
//!
//! An authored path is the text a user, setting or file wrote. These functions
//! join, normalize, resolve and split that text without touching the file
//! system or the process working directory. The string operations follow the
//! JavaScript runtime's `path` module for both of its flavors, except where
//! that module's result is a defect; the crate's tests mark each one.
//! [`native_home_dir`] alone reads the environment and native account home;
//! [`home_directory`] is its text form and [`home_directory_text`] converts a supplied home.
//! Convert an authored path to [`std::path::PathBuf`] only at a file-system call.
//! [`normalize_native`] shares segment reduction with these string operations
//! when a native filesystem path must retain non-UTF-8 components.
//! [`resolve_final_links`] traverses native link targets supplied by the caller.
//! The [`fs`] module alone reads the file system and the process directories: it
//! resolves a path against the working directory and the Windows drive
//! directories, and on native targets it lists a directory in name order and
//! resolves real paths.
//!
//! The [`posix`] and [`win32`] modules behave the same on every platform. The
//! crate root re-exports the flavor of the compile target; the POSIX-only
//! [`posix::parse`] is also re-exported on targets other than Windows.

pub mod fs;
mod links;
mod os;
mod path;
mod tilde;

pub use links::resolve_final_links;
pub use os::{home_dir, home_directory, home_directory_text, native_home_dir};
pub use path::{
    Cwd, CwdUnavailable, normalize_native, posix, resolve_with, resolve_with_flavor, slashed, win32,
};
pub use tilde::{
    Tilde, expand_tilde, expand_tilde_joined, expand_tilde_native, join_home, parse_tilde,
    shorten_home, shorten_home_normalized,
};

#[cfg(not(windows))]
pub use path::posix::{
    ParsedPath, SEP, basename, dirname, is_absolute, is_separator, join, normalize, parse,
    relative, resolve, try_resolve,
};
#[cfg(windows)]
pub use path::win32::{
    SEP, basename, dirname, is_absolute, is_separator, join, normalize, relative, resolve,
    try_resolve,
};

/// Read each drive's `=X:` variable through `var`, in letter order.
pub fn env_drive_directories(
    var: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Vec<(char, String)> {
    ('A'..='Z')
        .filter_map(|letter| {
            var(&format!("={letter}:")).map(|dir| (letter, dir.to_string_lossy().into_owned()))
        })
        .collect()
}
