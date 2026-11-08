//! The standalone account file: stored credentials by provider, as one JSON object.
use serde_json::{Map, Value};

/// Why an account file was not saved.
#[derive(Debug)]
pub enum AccountFileError {
    /// The stored root is valid JSON but not an object; the file is left unchanged.
    NotAnObject,
    /// The file could not be written.
    Write(std::io::Error),
}

impl std::fmt::Display for AccountFileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAnObject => {
                formatter.write_str("The account file does not hold a JSON object")
            }
            Self::Write(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AccountFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotAnObject => None,
            Self::Write(error) => Some(error),
        }
    }
}

/// The stored credentials of the account file at `path`, by provider, in stored order; a
/// missing, unreadable or unparsable file, or one whose root is not an object, gives none.
#[must_use]
pub fn load_auth(path: &str) -> Map<String, Value> {
    match stored(path) {
        Some(Value::Object(members)) => members,
        _ => Map::new(),
    }
}

/// Writes `auth` to the account file at `path` as two-space JSON with no final newline, in
/// the canonical member order and number spelling. The file is written in place under the
/// shared recoverable publication and readable only by its owner.
///
/// # Errors
/// Returns [`AccountFileError::NotAnObject`] without writing when the file holds valid JSON
/// whose root is not an object, and the write failure otherwise.
pub fn save_auth(path: &str, auth: &Map<String, Value>) -> Result<(), AccountFileError> {
    if stored(path).is_some_and(|root| !root.is_object()) {
        return Err(AccountFileError::NotAnObject);
    }
    let text = maestro_json::pretty_object(auth)
        .map_err(|error| AccountFileError::Write(std::io::Error::other(error)))?;
    write_private(path, &text).map_err(AccountFileError::Write)
}

/// Publishes `text` as the credential file at `path`, readable only by its owner on Unix.
///
/// The text is written in place under the shared exclusion, so a hard link to the file sees it,
/// and a write cut short is completed, not rolled back, by the next coordinated writer. A symbolic
/// link at `path` is followed and no directory is created. An existing file must be writable by
/// its owner: it is narrowed to mode 0600 through the handle that proved this, so the new text is
/// never written under looser bits, and a read-only file is refused without a change. A failed
/// parent-directory sync after the write is not reported: the text is already written.
///
/// # Errors
/// Returns the native error of the narrowing or of the publication.
pub(crate) fn write_private(path: &str, text: &str) -> std::io::Result<()> {
    #[cfg(unix)]
    maestro_lock::narrow_existing(std::path::Path::new(path), 0o600)?;
    maestro_lock::replace(
        std::path::Path::new(path),
        text.as_bytes(),
        maestro_lock::Replace {
            through_symlinks: true,
            creation_mode: Some(maestro_lock::CreationMode::AfterSync(0o600)),
        },
    )
    .map(|_| ())
}

/// The parsed root of the file at `path`, absent when it cannot be read or parsed.
fn stored(path: &str) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}
