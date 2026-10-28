//! Worktree commands: listing, opening, creating and removing linked checkouts.

mod create;
mod info;
mod list;
mod open;
mod operations;
mod remove;
mod source;

use super::responses::api_error;
use crate::paths::expand_home_path;
use maestro_protocol::schema::ErrorBody;
use std::path::PathBuf;

pub(in crate::app) use operations::WorktreeOperations;

/// The user's home directory, when the environment names one.
fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// A path the caller spelled, with a leading `~` expanded, which must be absolute.
fn absolute_user_path(path: &str) -> Result<PathBuf, ErrorBody> {
    let path = expand_home_path(path, home);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(api_error(
            "invalid_request",
            "worktree path must be absolute",
        ))
    }
}

/// The refusal for a checkout another worktree request is already working on.
fn operation_in_progress() -> ErrorBody {
    api_error(
        "worktree_operation_in_progress",
        "worktree operation is already in progress for this checkout",
    )
}
