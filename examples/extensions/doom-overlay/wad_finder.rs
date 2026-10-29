//! Native WAD discovery and the optional shareware download.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult, http::Request, tools::path_utils};
use std::io::Write;

mod checksum;
use checksum::matches_checksum;

/// Shareware data downloaded only when no local candidate exists.
const WAD_URL: &str = "https://raw.githubusercontent.com/Daivuk/PureDOOM/355cfbd16fac119718879239336ee2ea408886bd/doom1.wad";

/// Official shareware v1.9 bytes expected from the pinned mirror.
const WAD_SHA256: &str = "1d7d43be501e67d927e415e0b8f3e29c3bf33075e859721816f652a526cac771";

/// Finds a custom file, or the first module, working-directory or home candidate.
/// # Errors
/// Returns the native path owner's failure.
pub fn find_wad_file(
    api: &ExtensionAPI,
    cwd: &str,
    custom: Option<&str>,
) -> ExtensionResult<Option<String>> {
    if let Some(custom) = custom {
        let path = path_utils::resolve_to_cwd(api, custom, cwd)?;
        return Ok(is_file(api, &path)?.then_some(path));
    }
    let bundled = bundled_wad(api)?;
    if is_file(api, &bundled)? {
        return Ok(Some(bundled));
    }
    for candidate in [
        "./doom1.wad",
        "./DOOM1.WAD",
        "~/doom1.wad",
        "~/.doom/doom1.wad",
    ] {
        let path = path_utils::resolve_to_cwd(api, candidate, cwd)?;
        if is_file(api, &path)? {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// Returns a local WAD or caches the complete downloaded body beside this artifact.
/// # Errors
/// Returns discovery, HTTP, body or filesystem failures without caching partial responses.
pub async fn ensure_wad_file(api: &ExtensionAPI, cwd: &str) -> ExtensionResult<String> {
    if let Some(path) = find_wad_file(api, cwd, None)? {
        return Ok(path);
    }
    let response = api
        .native_io()
        .fetch(Request {
            url: WAD_URL.into(),
            method: None,
            headers: Vec::new(),
            body: None,
            signal: None,
        })
        .await?;
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    let body = response.body().ok_or("The WAD response has no body")?;
    let mut bytes = Vec::new();
    while let Some(mut chunk) = body.read().await? {
        bytes.append(&mut chunk);
    }
    if bytes.is_empty() {
        return Err("The WAD response is empty".into());
    }
    if !matches_checksum(&bytes, WAD_SHA256) {
        return Err("The downloaded WAD checksum does not match shareware v1.9".into());
    }
    let path = bundled_wad(api)?;
    write_cache(&path_utils::guest_path(api, &path)?, &bytes)?;
    Ok(path)
}

/// Names the local cache without depending on the process working directory.
fn bundled_wad(api: &ExtensionAPI) -> ExtensionResult<String> {
    let artifact = std::env::var("MAESTRO_EXTENSION_PATH").map_err(|e| e.to_string())?;
    path_utils::resolve(api, &[&artifact, "..", "doom1.wad"])
}

/// Rejects directories rather than selecting an unreadable WAD candidate.
fn is_file(api: &ExtensionAPI, path: &str) -> ExtensionResult<bool> {
    Ok(std::fs::metadata(path_utils::guest_path(api, path)?)
        .is_ok_and(|metadata| metadata.is_file()))
}

/// Creates only a new cache file, removing its own bytes if the write fails.
fn write_cache(path: &str, bytes: &[u8]) -> ExtensionResult<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    if let Err(error) = file.write_all(bytes) {
        drop(file);
        std::fs::remove_file(path)
            .map_err(|cleanup| format!("{error}; removing partial WAD failed: {cleanup}"))?;
        return Err(error.to_string());
    }
    Ok(())
}
