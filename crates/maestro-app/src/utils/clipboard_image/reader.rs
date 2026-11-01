//! Owned image reading through replaceable clipboard effects.
use super::{base_mime_type, is_wayland_session};
use crate::utils::clipboard::{
    ClipboardOperations, ClipboardPlatform, NativeClipboardOperations, environment,
};
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};

/// Owned clipboard image data.
#[derive(Debug)]
pub struct ClipboardImage {
    /// Image bytes.
    pub bytes: Vec<u8>,
    /// MIME label accompanying the bytes.
    pub mime_type: String,
}
/// Routing overrides; absent fields observe the host independently.
#[derive(Default)]
pub struct ReadClipboardImageOptions {
    /// Complete routing environment, not a helper environment override.
    pub env: Option<BTreeMap<String, String>>,
    /// Routing platform, without changing native admission.
    pub platform: Option<ClipboardPlatform>,
}
/// Filesystem effects used by clipboard image routes.
pub trait ClipboardImageOperations: ClipboardOperations {
    /// Reads raw file bytes.
    ///
    /// # Errors
    /// Returns the original filesystem failure.
    fn read_file(&self, path: &Path) -> io::Result<Vec<u8>>;
    /// Removes a temporary image.
    ///
    /// # Errors
    /// Returns the original filesystem failure.
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    /// Produces a fresh temporary PNG path.
    fn temporary_png_path(&self) -> PathBuf;
}
impl ClipboardImageOperations for NativeClipboardOperations {
    fn read_file(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }
    fn remove_file(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }
    fn temporary_png_path(&self) -> PathBuf {
        std::env::temp_dir().join(format!("maestro-wsl-clip-{}.png", uuid::Uuid::new_v4()))
    }
}
/// Reads through the host clipboard adapter.
///
/// # Errors
/// Returns native clipboard probe or read failures unchanged.
pub async fn read_clipboard_image(
    options: Option<&ReadClipboardImageOptions>,
) -> io::Result<Option<ClipboardImage>> {
    read_clipboard_image_with(options, &NativeClipboardOperations).await
}
/// Reads an owned clipboard image using supplied effects.
///
/// # Errors
/// Returns native clipboard probe or read failures unchanged.
pub async fn read_clipboard_image_with(
    options: Option<&ReadClipboardImageOptions>,
    operations: &impl ClipboardImageOperations,
) -> io::Result<Option<ClipboardImage>> {
    let env = |name: &str| match options.and_then(|options| options.env.as_ref()) {
        Some(env) => env.get(name).cloned(),
        None => environment(name),
    };
    if env("TERMUX_VERSION").is_some_and(|value| !value.is_empty()) {
        return Ok(None);
    }
    let platform = options
        .and_then(|options| options.platform)
        .unwrap_or_else(|| operations.platform());
    if platform == ClipboardPlatform::Linux {
        let wsl = is_wsl(operations, env);
        let wayland = is_wayland_session(env);
        if (wayland || wsl)
            && let Some(image) = read_clipboard_image_via_wl_paste(operations)
                .or_else(|| read_clipboard_image_via_xclip(operations))
        {
            return Ok(convert_image(image));
        }
        if wsl && let Some(image) = read_clipboard_image_via_powershell(operations) {
            return Ok(convert_image(image));
        }
        if wayland {
            return Ok(None);
        }
    }
    Ok(read_clipboard_image_via_native_clipboard(operations)
        .await?
        .and_then(convert_image))
}
/// Completes the native probe and read before returning owned bytes.
async fn read_clipboard_image_via_native_clipboard(
    operations: &impl ClipboardOperations,
) -> io::Result<Option<ClipboardImage>> {
    let Some(module) = operations.clipboard() else {
        return Ok(None);
    };
    if !module.has_image()? {
        return Ok(None);
    }
    Ok(module
        .get_image_binary()
        .await?
        .filter(|bytes| !bytes.is_empty())
        .map(|bytes| ClipboardImage {
            bytes,
            mime_type: base_mime_type("image/png"),
        }))
}

/// Runs a bounded helper with closed stdin and captured descriptors.
fn run_command(
    operations: &impl ClipboardOperations,
    program: &str,
    args: &[&str],
    timeout_ms: u64,
) -> Option<Vec<u8>> {
    use maestro_process::{OutputMode, SpawnSyncOptions};
    operations
        .spawn_sync(
            maestro_process::command(program).args(args),
            SpawnSyncOptions::Capture {
                stdout: OutputMode::Capture,
                stderr: OutputMode::Capture,
                input: Some(&[]),
                timeout: Some(std::time::Duration::from_millis(timeout_ms)),
                max_buffer_bytes: Some(50 * 1024 * 1024),
            },
        )
        .ok()
        .filter(|output| output.status.success())
        .map(|output| output.stdout)
}
/// Selects supported types first while retaining raw labels and parameters.
fn select_preferred_image_mime_type(types: &str) -> Option<&str> {
    let types: Vec<_> = types
        .split('\n')
        .map(super::trim)
        .filter(|mime| !mime.is_empty())
        .collect();
    super::SUPPORTED
        .iter()
        .find_map(|preferred| {
            types
                .iter()
                .copied()
                .find(|mime| base_mime_type(mime) == *preferred)
        })
        .or_else(|| {
            types
                .into_iter()
                .find(|mime| base_mime_type(mime).starts_with("image/"))
        })
}
/// Returns nonempty helper output with its normalized MIME label.
fn helper_image(bytes: Vec<u8>, mime: &str) -> Option<ClipboardImage> {
    (!bytes.is_empty()).then(|| ClipboardImage {
        bytes,
        mime_type: base_mime_type(mime),
    })
}
/// Reads the preferred Wayland target.
fn read_clipboard_image_via_wl_paste(
    operations: &impl ClipboardOperations,
) -> Option<ClipboardImage> {
    let types = run_command(operations, "wl-paste", &["--list-types"], 1000)?;
    let types = String::from_utf8_lossy(&types);
    let mime = select_preferred_image_mime_type(&types)?;
    helper_image(
        run_command(
            operations,
            "wl-paste",
            &["--type", mime, "--no-newline"],
            3000,
        )?,
        mime,
    )
}
/// Detects WSL using routing flags before reading the kernel version.
fn is_wsl(
    operations: &impl ClipboardImageOperations,
    env: impl Fn(&str) -> Option<String>,
) -> bool {
    if ["WSL_DISTRO_NAME", "WSLENV"]
        .iter()
        .any(|name| env(name).is_some_and(|value| !value.is_empty()))
    {
        return true;
    }
    operations
        .read_file(Path::new("/proc/version"))
        .is_ok_and(|bytes| {
            let text = String::from_utf8_lossy(&bytes).to_lowercase();
            text.contains("microsoft") || text.contains("wsl")
        })
}

/// Probes the preferred X11 target followed by every supported type, without deduplication.
fn read_clipboard_image_via_xclip(operations: &impl ClipboardOperations) -> Option<ClipboardImage> {
    let targets = run_command(
        operations,
        "xclip",
        &["-selection", "clipboard", "-t", "TARGETS", "-o"],
        1000,
    )
    .unwrap_or_default();
    let targets = String::from_utf8_lossy(&targets);
    for mime in select_preferred_image_mime_type(&targets)
        .into_iter()
        .chain(super::SUPPORTED)
    {
        if let Some(image) = run_command(
            operations,
            "xclip",
            &["-selection", "clipboard", "-t", mime, "-o"],
            3000,
        )
        .and_then(|bytes| helper_image(bytes, mime))
        {
            return Some(image);
        }
    }
    None
}

/// Reads a Windows clipboard image through a temporary PNG and always attempts cleanup.
fn read_clipboard_image_via_powershell(
    operations: &impl ClipboardImageOperations,
) -> Option<ClipboardImage> {
    let path = operations.temporary_png_path();
    let image = powershell_file(operations, &path);
    let _ = operations.remove_file(&path);
    image
}
/// Converts the supplied path before invoking the Windows image API.
fn powershell_file(
    operations: &impl ClipboardImageOperations,
    path: &Path,
) -> Option<ClipboardImage> {
    let win_path = run_command(operations, "wslpath", &["-w", path.to_str()?], 1000)?;
    let win_path = String::from_utf8_lossy(&win_path);
    let win_path = super::trim(&win_path);
    if win_path.is_empty() {
        return None;
    }
    let quoted = maestro_process::shell::quote_powershell_literal(win_path);
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; Add-Type -AssemblyName System.Drawing; $path = {quoted}; $img = [System.Windows.Forms.Clipboard]::GetImage(); if ($img) {{ $img.Save($path, [System.Drawing.Imaging.ImageFormat]::Png); Write-Output 'ok' }} else {{ Write-Output 'empty' }}"
    );
    let output = run_command(
        operations,
        "powershell.exe",
        &["-NoProfile", "-Command", &script],
        5000,
    )?;
    if super::trim(&String::from_utf8_lossy(&output)) != "ok" {
        return None;
    }
    helper_image(operations.read_file(path).ok()?, "image/png")
}

/// Passes supported labels through and converts other formats without rotating pixels.
fn convert_image(image: ClipboardImage) -> Option<ClipboardImage> {
    if super::SUPPORTED.contains(&image.mime_type.as_str()) {
        return Some(image);
    }
    helper_image(
        maestro_tools::utils::image_convert::convert_bytes_to_png(&image.bytes)?,
        "image/png",
    )
}
