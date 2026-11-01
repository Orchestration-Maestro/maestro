//! Lexical operations on authored path strings.
//!
//! An authored path is the text a user, setting or file wrote. These functions
//! join, normalize, resolve and split that text without touching the file
//! system or the process working directory. The string operations follow the
//! JavaScript runtime's `path` module for both of its flavors, except where
//! that module's result is a defect; the crate's tests mark each one.
//! [`native_homedir`] alone reads the environment and native account home. Convert
//! an authored path to [`std::path::PathBuf`] only at a file-system call.
//! [`normalize_native`] shares segment reduction with these string operations
//! when a native filesystem path must retain non-UTF-8 components.
//! [`resolve_final_links`] traverses native link targets supplied by the caller.
//!
//! The [`posix`] and [`win32`] modules behave the same on every platform. The
//! crate root re-exports the flavor of the compile target; the POSIX-only
//! [`posix::parse`] is also re-exported on targets other than Windows.

mod links;
mod os;
mod path;
mod tilde;

pub use links::resolve_final_links;
pub use os::{homedir, native_homedir};
pub use path::{
    Cwd, CwdUnavailable, normalize_native, posix, resolve_with, resolve_with_flavor, win32,
};
pub use tilde::{
    Tilde, expand_tilde, expand_tilde_joined, expand_tilde_native, expand_tilde_slash_joined,
    parse_tilde,
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
