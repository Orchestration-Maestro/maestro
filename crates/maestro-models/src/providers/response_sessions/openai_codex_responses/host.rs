//! Host identity and native request identifiers.

/// Branded identity of the current execution target.
pub(super) fn user_agent() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        "maestro (browser)".to_owned()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (platform, arch) = node_names(std::env::consts::OS, std::env::consts::ARCH);
        format!("maestro ({platform} {}; {arch})", release())
    }
}

/// Spelling of a Rust target operating system and architecture in `node:os`.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn node_names<'a>(os: &'a str, arch: &'a str) -> (&'a str, &'a str) {
    let platform = match os {
        "macos" => "darwin",
        "windows" => "win32",
        "solaris" | "illumos" => "sunos",
        os => os,
    };
    let arch = match arch {
        "x86_64" => "x64",
        "x86" => "ia32",
        "aarch64" => "arm64",
        "powerpc64" => "ppc64",
        "loongarch64" => "loong64",
        arch => arch,
    };
    (platform, arch)
}

/// Kernel release in the native operating system's spelling.
#[cfg(unix)]
fn release() -> String {
    rustix::system::uname()
        .release()
        .to_string_lossy()
        .into_owned()
}

/// Windows release, excluding service-pack and revision fields.
#[cfg(windows)]
fn release() -> String {
    let version = windows_version::OsVersion::current();
    format!("{}.{}.{}", version.major, version.minor, version.build)
}

/// Canonical random v4 identifier; randomness failures remain fallible.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn request_id() -> Result<String, crate::DiagnosticErrorInfo> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|error| super::request::diagnostic(error.to_string()))?;
    Ok(uuid::Builder::from_random_bytes(bytes)
        .into_uuid()
        .to_string())
}
