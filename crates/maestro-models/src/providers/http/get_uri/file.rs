//! Opened-file identity, reads and millisecond modification observations.

use std::io::Read;

use super::{Cache, Failure, Source, notfound::NotFound, notmodified::NotModified};

/// Keep filesystem work off the asynchronous executor and close the same opened handle.
pub(super) async fn load(uri: &url::Url, cache: Option<&Cache>) -> Result<Source, Failure> {
    let encoded = uri.path().to_ascii_lowercase();
    if encoded.contains("%2f") || (cfg!(windows) && encoded.contains("%5c")) {
        return Err(std::io::Error::other("Encoded separator in file URL").into());
    }
    let path = uri
        .to_file_path()
        .map_err(|()| std::io::Error::other("Invalid file URL"))?;
    let previous = match cache {
        Some(Cache::File(modified)) => Some(*modified),
        _ => None,
    };
    tokio::task::spawn_blocking(move || {
        let mut file = std::fs::File::open(&path).map_err(|error| not_found(error, &path))?;
        let modified = file
            .metadata()?
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .map_err(|error| error.duration().as_millis());
        if previous == Some(modified) {
            return Err(NotModified.into());
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(Source {
            bytes,
            cache: Cache::File(modified),
        })
    })
    .await?
}

/// Missing files alone have the source's authored not-found category. Windows refuses to
/// open an existing directory named with a trailing separator as not found, so the existence
/// of the entry the open would reach, following links, decides.
fn not_found(error: std::io::Error, path: &std::path::Path) -> Failure {
    if error.kind() == std::io::ErrorKind::NotFound && std::fs::metadata(path).is_err() {
        NotFound.into()
    } else {
        error.into()
    }
}
