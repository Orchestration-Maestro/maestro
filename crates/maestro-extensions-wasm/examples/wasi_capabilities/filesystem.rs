//! Filesystem operations through the ordinary and asynchronous standard interfaces.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::wasi;
use serde_json::{Value, json};
use wasi::filesystem::types::{DescriptorFlags, ErrorCode, OpenFlags, PathFlags};

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
        let result = ordinary(path, &bytes, write);
        Ok(result.map_or_else(
            |error| json!({"error":ordinary_error(&error)}),
            |bytes| json!({"bytes":bytes}),
        ))
    } else {
        let result = asynchronous(path, bytes, write).await;
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

/// Opens the path below its matching root through the native async filesystem implementation.
async fn asynchronous(path: &str, bytes: Vec<u8>, write: bool) -> Result<Vec<u8>, ErrorCode> {
    let absolute = if path.starts_with('/') {
        path.to_owned()
    } else {
        format!(
            "{}/{path}",
            wasi::cli::environment::get_initial_cwd().ok_or(ErrorCode::NoEntry)?
        )
    };
    let roots = wasi::filesystem::preopens::get_directories();
    // The longest root name that ends at a separator, as the ordinary libc selects it.
    let (root, name) = roots
        .iter()
        .map(|(root, name)| (root, name.trim_end_matches('/')))
        .filter(|(_, name)| {
            absolute.starts_with(name)
                && matches!(absolute.as_bytes().get(name.len()), None | Some(b'/'))
        })
        .max_by_key(|(_, name)| name.len())
        .ok_or(ErrorCode::NoEntry)?;
    let path = absolute[name.len()..].trim_start_matches('/');
    let path = if path.is_empty() { "." } else { path }.to_owned();
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
