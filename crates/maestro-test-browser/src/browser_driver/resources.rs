//! Resolve packaged browser resources without exposing the checkout.

use hyper::StatusCode;
use percent_encoding::percent_decode_str;
use sha2::{Digest, Sha256};
use std::{fs, io, path::Path};

/// Contribution-page resource policy, including independently loaded WebAssembly modules.
pub(super) const POLICY: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; object-src 'none'";

/// A resource response independent of the HTTP transport.
#[derive(Debug)]
pub(super) struct Resource {
    /// HTTP status code.
    pub status: StatusCode,
    /// Content policy attached to the decoded contribution route.
    pub policy: Option<&'static str>,
    /// Optional MIME type for successful reads.
    pub mime: Option<&'static str>,
    /// Served bytes, empty on refused or missing routes.
    pub bytes: Vec<u8>,
}

/// Read one request path beneath the canonical packaged root.
pub(super) fn read(root: &Path, request: &str) -> Resource {
    match resolve(root, request) {
        Ok(resource) => resource,
        Err(error) => Resource {
            status: if error.kind() == io::ErrorKind::NotFound {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::FORBIDDEN
            },
            policy: None,
            mime: None,
            bytes: Vec::new(),
        },
    }
}

/// Decode one resource URL path and refuse traversal, NUL, backslashes and malformed escapes.
///
/// # Errors
/// Returns malformed UTF-8 or URL escapes and refused path components.
pub fn resource_path(request: &str) -> io::Result<String> {
    let raw = request.split('?').next().unwrap_or("");
    for (index, byte) in raw.bytes().enumerate() {
        if byte == b'%'
            && !raw
                .as_bytes()
                .get(index + 1..index + 3)
                .is_some_and(|bytes| bytes.iter().all(u8::is_ascii_hexdigit))
        {
            return Err(io::Error::other("malformed URL escape"));
        }
    }
    let path = percent_decode_str(raw)
        .decode_utf8()
        .map_err(io::Error::other)?
        .into_owned();
    if path.contains(['\\', '\0']) || path.split('/').any(|part| part == "..") {
        return Err(io::Error::other("refused resource path"));
    }
    Ok(path)
}

/// Return canonical packaged bytes, retaining helper aliases before following links.
fn resolve(root: &Path, request: &str) -> io::Result<Resource> {
    let root = maestro_path::fs::canonicalize(root)?;
    let root_text = root
        .to_str()
        .ok_or_else(|| io::Error::other("resource root requires UTF-8"))?;
    let path = resource_path(request)?;
    if path == "/contributions.html" {
        return Ok(Resource {
            status: StatusCode::OK,
            policy: Some(POLICY),
            mime: Some("text/html"),
            bytes: b"<!doctype html><body></body>".to_vec(),
        });
    }
    let relative = if path == "/" {
        "index.html"
    } else {
        path.strip_prefix('/').unwrap_or(&path)
    };
    let mut file = maestro_path::resolve(
        &[root_text, relative],
        &maestro_path::Cwd {
            current: root_text,
            drive_directories: &[],
        },
    );
    if !Path::new(&file).starts_with(&root) || Path::new(&file) == root {
        return Err(io::Error::other("path outside packaged root"));
    }
    if maestro_path::basename(&file, None) == "helper.js" {
        file = maestro_path::join(&[root_text, "helper.js"]);
    }
    let file = maestro_path::fs::canonicalize(&file)?;
    if !file.starts_with(&root) || file == root {
        return Err(io::Error::other("path outside packaged root"));
    }
    let mime = match file.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html",
        Some("js") => "application/javascript",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    };
    Ok(Resource {
        status: StatusCode::OK,
        policy: None,
        mime: Some(mime),
        bytes: fs::read(file)?,
    })
}

/// Hash host files while excluding independently loaded module packages.
pub(super) fn digest(root: &Path) -> io::Result<String> {
    let mut files = Vec::new();
    collect(root, "", &mut files)?;
    files.sort();
    let mut digest = Sha256::new();
    for file in files {
        digest.update(file.as_bytes());
        digest.update(fs::read(root.join(file))?);
    }
    Ok(maestro_tooling::hex(&digest.finalize()))
}

/// Collect host-relative names in the same order on every platform.
fn collect(root: &Path, relative: &str, files: &mut Vec<String>) -> io::Result<()> {
    for entry in fs::read_dir(root.join(relative))? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| io::Error::other("host filename requires UTF-8"))?;
        let name = if relative.is_empty() {
            name
        } else {
            format!("{relative}/{name}")
        };
        if name.starts_with("package") {
            continue;
        }
        if entry.file_type()?.is_dir() {
            collect(root, &name, files)?;
        } else if fs::metadata(entry.path())?.is_file() {
            files.push(name);
        }
    }
    Ok(())
}
