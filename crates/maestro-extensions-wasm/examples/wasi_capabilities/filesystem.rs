//! Filesystem operations through the ordinary and asynchronous standard interfaces.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::wasi;
use serde_json::{Value, json};
use wasi::filesystem::types::{Descriptor, DescriptorFlags, ErrorCode, OpenFlags, PathFlags};

/// Reports an unavailable cwd without preventing absolute filesystem operations.
pub(super) fn cwd() -> Value {
    let ordinary = std::env::current_dir().map_or_else(
        |error| json!({"error":ordinary_error(&error)}),
        |path| json!({"path":path}),
    );
    json!({"p2":ordinary,"p3":wasi::cli::environment::get_initial_cwd()})
}

/// Performs an actual write/read or read of the guest-spelled path and reports OS failures.
pub(super) async fn operate(request: &Value, path: &str) -> Result<Value, String> {
    let write = request["write"].as_bool().unwrap_or(false);
    let bytes: Vec<u8> =
        serde_json::from_value(request["bytes"].clone()).map_err(|error| error.to_string())?;
    if request["generation"] == "p2" {
        let result = match request["action"].as_str() {
            Some("unlink") => std::fs::remove_file(path).map(|()| Vec::new()),
            Some("exclusive") => std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map(|_| Vec::new()),
            _ => ordinary(path, &bytes, write),
        };
        Ok(result.map_or_else(
            |error| json!({"error":ordinary_error(&error)}),
            |bytes| json!({"bytes":bytes}),
        ))
    } else {
        let result = match request["action"].as_str() {
            Some("unlink") => {
                let (root, name) = below_root(path).map_err(|error| format!("{error:?}"))?;
                root.unlink_file_at(name).await.map(|()| Vec::new())
            }
            Some("exclusive") => {
                let (root, name) = below_root(path).map_err(|error| format!("{error:?}"))?;
                root.open_at(
                    PathFlags::SYMLINK_FOLLOW,
                    name,
                    OpenFlags::CREATE | OpenFlags::EXCLUSIVE,
                    DescriptorFlags::WRITE,
                )
                .await
                .map(|_| Vec::new())
            }
            _ => asynchronous(path, bytes, write).await,
        };
        Ok(result.map_or_else(
            |error| json!({"error":asynchronous_error(&error),"code":format!("{error:?}")}),
            |bytes| json!({"bytes":bytes}),
        ))
    }
}

/// Uses the standard library's actual cwd, preopens, permissions and symlink resolution.
fn ordinary(path: &str, bytes: &[u8], write: bool) -> std::io::Result<Vec<u8>> {
    if write {
        std::fs::write(path, bytes)?;
    }
    std::fs::read(path)
}

/// Creates a link to `target` at `link` and reads its target and the bytes behind it.
///
/// The ordinary standard library has no stable way to create a link on this
/// target, so the ordinary generation reads a link the host made at `link`.
pub(super) async fn link(request: &Value, target: &str, link: &str) -> Result<Value, String> {
    if request["generation"] == "p2" {
        let result = std::fs::read_link(link).and_then(|target| Ok((target, std::fs::read(link)?)));
        return Ok(result.map_or_else(
            |error| json!({"error":ordinary_error(&error)}),
            |(target, bytes)| json!({"target":target,"bytes":bytes}),
        ));
    }
    let result = async {
        let (root, path) = below_root(link)?;
        root.symlink_at(target.to_owned(), path.clone()).await?;
        let target = root.readlink_at(path).await?;
        Ok((target, asynchronous(link, Vec::new(), false).await?))
    }
    .await;
    Ok(result.map_or_else(
        |error: ErrorCode| json!({"error":asynchronous_error(&error),"code":format!("{error:?}")}),
        |(target, bytes)| json!({"target":target,"bytes":bytes}),
    ))
}

/// The root that holds `path` and the path below it, as the ordinary libc selects them.
fn below_root(path: &str) -> Result<(Descriptor, String), ErrorCode> {
    let absolute = if path.starts_with('/') {
        path.to_owned()
    } else {
        format!(
            "{}/{path}",
            wasi::cli::environment::get_initial_cwd().ok_or(ErrorCode::NoEntry)?
        )
    };
    // The longest root name that ends at a separator.
    let (root, name) = wasi::filesystem::preopens::get_directories()
        .into_iter()
        .filter(|(_, name)| {
            let name = name.trim_end_matches('/');
            absolute.starts_with(name)
                && matches!(absolute.as_bytes().get(name.len()), None | Some(b'/'))
        })
        .max_by_key(|(_, name)| name.trim_end_matches('/').len())
        .ok_or(ErrorCode::NoEntry)?;
    let path = absolute[name.trim_end_matches('/').len()..].trim_start_matches('/');
    Ok((root, if path.is_empty() { "." } else { path }.to_owned()))
}

/// Opens the path below its matching root through the native async filesystem implementation.
async fn asynchronous(path: &str, bytes: Vec<u8>, write: bool) -> Result<Vec<u8>, ErrorCode> {
    let (root, path) = below_root(path)?;
    let flags = if write {
        OpenFlags::CREATE | OpenFlags::TRUNCATE
    } else {
        OpenFlags::empty()
    };
    let access = if write {
        DescriptorFlags::READ | DescriptorFlags::WRITE
    } else {
        DescriptorFlags::READ
    };
    let file = root
        .open_at(PathFlags::SYMLINK_FOLLOW, path, flags, access)
        .await?;
    if write {
        let (mut writer, reader) = wasi::wit_stream::new::<u8>();
        let completion = file.write_via_stream(reader, 0);
        let remaining = writer.write_all(bytes).await;
        drop(writer);
        completion.await?;
        if !remaining.is_empty() {
            return Err(ErrorCode::Io);
        }
    }
    let (body, completion) = file.read_via_stream(0);
    let bytes = body.collect().await;
    completion.await?;
    Ok(bytes)
}

/// Stable distinctions checked by the host, not engine-specific error strings.
fn ordinary_error(error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::NotFound => "not-found".into(),
        std::io::ErrorKind::PermissionDenied => "permission-denied".into(),
        kind => format!("{kind:?}"),
    }
}

/// The corresponding native asynchronous error distinctions.
fn asynchronous_error(error: &ErrorCode) -> String {
    match error {
        ErrorCode::NoEntry => "not-found".into(),
        ErrorCode::Access | ErrorCode::NotPermitted => "permission-denied".into(),
        error => format!("{error:?}"),
    }
}
