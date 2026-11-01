//! Native search program discovery and release acquisition.

use futures_util::TryStreamExt;
use maestro_models::{Fetch, FetchError, HttpRequest, default_fetch};
use maestro_process::child_process::{CapturedChild, WaitOptions, wait_for_child_process};
use maestro_text::is_whitespace_scalar;
use owo_colors::OwoColorize;
use serde_json::Value;
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};

/// Deadline of the release lookup, response body included.
const NETWORK_TIMEOUT: Duration = Duration::from_millis(10_000);
/// Deadline of an archive download, response body included.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_millis(120_000);
/// User agent of the release lookup.
const USER_AGENT: &str = "maestro-coding-agent";

/// A native search program the find and grep tools run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchTool {
    /// `fd`, which finds files by name.
    Fd,
    /// `rg` (ripgrep), which searches file contents.
    Rg,
}

/// Where a search program comes from and how it is named.
struct ToolConfig {
    /// Name in progress lines and the Termux package name.
    name: &'static str,
    /// GitHub repository publishing the releases.
    repo: &'static str,
    /// Program name inside the archive and in the binary directory.
    binary_name: &'static str,
    /// Commands tried on `PATH`, in order.
    system_binary_names: &'static [&'static str],
    /// Prefix of release tags before the version.
    tag_prefix: &'static str,
}

/// The fd release source.
const FD: ToolConfig = ToolConfig {
    name: "fd",
    repo: "sharkdp/fd",
    binary_name: "fd",
    system_binary_names: &["fd", "fdfind"],
    tag_prefix: "v",
};

/// The ripgrep release source.
const RG: ToolConfig = ToolConfig {
    name: "ripgrep",
    repo: "BurntSushi/ripgrep",
    binary_name: "rg",
    system_binary_names: &["rg"],
    tag_prefix: "",
};

/// Archive format of a release asset.
#[derive(Clone, Copy)]
enum Archive {
    /// Gzip-compressed tar, unpacked by the `tar` command.
    TarGz,
    /// Zip, unpacked in process.
    Zip,
}

/// A release asset: its name without extension and its format.
struct Asset {
    /// Asset name without the archive extension; also the directory some archives nest under.
    stem: String,
    /// Archive format.
    archive: Archive,
}

impl Asset {
    /// Full asset file name.
    fn file_name(&self) -> String {
        let extension = match self.archive {
            Archive::TarGz => "tar.gz",
            Archive::Zip => "zip",
        };
        format!("{}.{extension}", self.stem)
    }
}

impl SearchTool {
    /// Release source and names of this program.
    const fn config(self) -> &'static ToolConfig {
        match self {
            Self::Fd => &FD,
            Self::Rg => &RG,
        }
    }

    /// The version fixed for `host` because later releases carry no build for it.
    fn pinned_version(self, host: Host) -> Option<&'static str> {
        (self == Self::Fd && host.os == "macos" && host.asset_arch() == "x86_64")
            .then_some("10.3.0")
    }

    /// The release asset for `host`, or `None` on an unsupported platform.
    fn asset(self, version: &str, host: Host) -> Option<Asset> {
        let arch = host.asset_arch();
        let (target, archive) = match (host.os, self) {
            ("macos", _) => ("apple-darwin", Archive::TarGz),
            ("linux", Self::Rg) if arch == "x86_64" => ("unknown-linux-musl", Archive::TarGz),
            ("linux", _) => ("unknown-linux-gnu", Archive::TarGz),
            ("windows", _) => ("pc-windows-msvc", Archive::Zip),
            _ => return None,
        };
        let prefix = match self {
            Self::Fd => format!("fd-v{version}"),
            Self::Rg => format!("ripgrep-{version}"),
        };
        Some(Asset {
            stem: format!("{prefix}-{arch}-{target}"),
            archive,
        })
    }
}

/// Operating system and architecture, as Rust names them.
#[derive(Clone, Copy)]
struct Host {
    /// `std::env::consts::OS` value.
    os: &'static str,
    /// `std::env::consts::ARCH` value.
    arch: &'static str,
}

impl Host {
    /// Architecture spelling used in release asset names.
    fn asset_arch(self) -> &'static str {
        match self.arch {
            "aarch64" => "aarch64",
            _ => "x86_64",
        }
    }
}

/// The host this build runs on.
const NATIVE_HOST: Host = Host {
    os: std::env::consts::OS,
    arch: std::env::consts::ARCH,
};

/// Where progress lines go and whether they are styled.
pub struct ToolsManagerOutput<'a> {
    /// Receives one line per progress or advice message.
    pub stdout: &'a mut (dyn Write + Send),
    /// Style lines with terminal colours.
    pub color: bool,
}

/// Style of a progress line.
#[derive(Clone, Copy)]
enum Tone {
    /// Progress.
    Dim,
    /// Advice and failure.
    Yellow,
}

/// Write `line` when `output` is present, styled by its colour flag.
fn say(output: &mut Option<ToolsManagerOutput<'_>>, tone: Tone, line: &str) {
    let Some(output) = output else { return };
    // A broken progress stream never changes whether a program is available.
    let _ = match (output.color, tone) {
        (false, _) => writeln!(output.stdout, "{line}"),
        (true, Tone::Dim) => writeln!(output.stdout, "{}", line.dimmed()),
        (true, Tone::Yellow) => writeln!(output.stdout, "{}", line.yellow()),
    };
}

/// Finds the search programs and installs missing ones into one binary directory.
pub struct ToolsManager {
    /// Directory holding installed programs, fixed at construction.
    bin_dir: String,
    /// HTTP client of release lookups and downloads.
    fetch: Fetch,
    /// Platform selecting assets and program names.
    host: Host,
    /// A caller's offline selection, or the ambient flag when absent.
    offline: Option<bool>,
}

impl ToolsManager {
    /// Manage programs in `bin_dir`, downloading through `fetch` or the default client.
    #[must_use]
    pub fn new(bin_dir: String, fetch: Option<Fetch>) -> Self {
        Self {
            bin_dir,
            fetch: fetch.unwrap_or_else(default_fetch),
            host: NATIVE_HOST,
            offline: None,
        }
    }

    /// Selects offline mode from the caller instead of the ambient environment.
    ///
    /// An absent selection keeps reading `MAESTRO_OFFLINE` when a download is considered.
    #[must_use]
    pub fn with_offline(mut self, offline: Option<bool>) -> Self {
        self.offline = offline;
        self
    }

    /// The program's path in the binary directory when it exists, else the first `PATH`
    /// command that starts, else `None`.
    pub async fn get_tool_path(&self, tool: SearchTool) -> Option<String> {
        let config = tool.config();
        let local = self.binary_path(config);
        if Path::new(&local).exists() {
            return Some(local);
        }
        for name in config.system_binary_names {
            if command_exists(name).await {
                return Some((*name).to_owned());
            }
        }
        None
    }

    /// The program's path, installing it when it is missing and installation is possible.
    pub async fn ensure_tool(
        &self,
        tool: SearchTool,
        output: Option<ToolsManagerOutput<'_>>,
    ) -> Option<String> {
        match self.get_tool_path(tool).await {
            Some(path) => Some(path),
            None => self.install_missing(tool, output).await,
        }
    }

    /// Report why a missing program stays missing, or download it.
    async fn install_missing(
        &self,
        tool: SearchTool,
        mut output: Option<ToolsManagerOutput<'_>>,
    ) -> Option<String> {
        let name = tool.config().name;
        if self.offline_mode_enabled() {
            let line = format!("{name} not found. Offline mode enabled, skipping download.");
            say(&mut output, Tone::Yellow, &line);
            return None;
        }
        if self.host.os == "android" {
            let line = format!("{name} not found. Install with: pkg install {name}");
            say(&mut output, Tone::Yellow, &line);
            return None;
        }
        let line = format!("{name} not found. Downloading...");
        say(&mut output, Tone::Dim, &line);
        match self.download_tool(tool).await {
            Ok(path) => {
                let line = format!("{name} installed to {path}");
                say(&mut output, Tone::Dim, &line);
                Some(path)
            }
            Err(error) => {
                let line = format!("Failed to download {name}: {error}");
                say(&mut output, Tone::Yellow, &line);
                None
            }
        }
    }

    /// Whether this manager refuses to download a missing program.
    fn offline_mode_enabled(&self) -> bool {
        maestro_text::is_offline_selected(self.offline)
    }

    /// File name of the program on this platform.
    fn binary_file_name(&self, config: &ToolConfig) -> String {
        let extension = match self.host.os {
            "windows" => ".exe",
            _ => "",
        };
        format!("{}{extension}", config.binary_name)
    }

    /// Path of the program in the binary directory.
    fn binary_path(&self, config: &ToolConfig) -> String {
        maestro_path::join(&[&self.bin_dir, &self.binary_file_name(config)])
    }

    /// Download, unpack and publish the latest release of `tool`, or its pinned version.
    ///
    /// Zip extraction and post-extraction steps do not yield; a dropped tar is killed.
    async fn download_tool(&self, tool: SearchTool) -> io::Result<String> {
        let config = tool.config();
        let version = match tool.pinned_version(self.host) {
            Some(pinned) => pinned.to_owned(),
            None => self.get_latest_version(config.repo).await?,
        };
        let asset = tool.asset(&version, self.host).ok_or_else(|| {
            io::Error::other(format!(
                "Unsupported platform: {}/{}",
                self.host.os, self.host.arch
            ))
        })?;
        std::fs::create_dir_all(&self.bin_dir)?;
        let staging = Staging::create(&self.bin_dir, config.binary_name)?;
        let file_name = asset.file_name();
        let url = format!(
            "https://github.com/{}/releases/download/{}{version}/{file_name}",
            config.repo, config.tag_prefix
        );
        let archive = maestro_path::join(&[&staging.path, &file_name]);
        let bytes = self
            .fetch_body(get(url, &[]), DOWNLOAD_TIMEOUT, "Failed to download")
            .await?;
        std::fs::write(&archive, bytes)?;
        match asset.archive {
            Archive::TarGz => extract_tar(&file_name, &archive, &staging.path).await?,
            Archive::Zip => zip::ZipArchive::new(std::fs::File::open(&archive)?)
                .and_then(|mut zip| zip.extract(&staging.path))
                .map_err(io::Error::other)?,
        }
        let binary = self.binary_file_name(config);
        let found = locate(&staging.path, &asset.stem, &binary)?;
        #[cfg(unix)]
        if self.host.os != "windows" {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&found, std::fs::Permissions::from_mode(0o755))?;
        }
        let target = self.binary_path(config);
        std::fs::rename(&found, &target)?;
        Ok(target)
    }

    /// The latest release version of `repo`, without one leading `v`.
    async fn get_latest_version(&self, repo: &str) -> io::Result<String> {
        let url = format!("https://api.github.com/repos/{repo}/releases/latest");
        let request = get(url, &[("user-agent", USER_AGENT)]);
        let body = self
            .fetch_body(request, NETWORK_TIMEOUT, "GitHub API error")
            .await?;
        let data: Value = serde_json::from_slice(&body)?;
        let tag = data
            .get("tag_name")
            .and_then(Value::as_str)
            .ok_or_else(|| io::Error::other("Release has no tag_name text"))?;
        Ok(tag.strip_prefix('v').unwrap_or(tag).to_owned())
    }

    /// The complete body of a successful response, within `deadline`; another status fails
    /// with `<failure>: <status>`.
    async fn fetch_body(
        &self,
        request: HttpRequest,
        deadline: Duration,
        failure: &str,
    ) -> io::Result<Vec<u8>> {
        let work = async {
            let response = (self.fetch)(request).await.map_err(io::Error::other)?;
            if !(200..300).contains(&response.status) {
                return Err(io::Error::other(format!("{failure}: {}", response.status)));
            }
            response.body.try_concat().await.map_err(io::Error::other)
        };
        tokio::time::timeout(deadline, work)
            .await
            .map_err(|_| io::Error::other(FetchError::Timeout))?
    }
}

/// A GET request with `headers`.
fn get(url: String, headers: &[(&str, &str)]) -> HttpRequest {
    HttpRequest {
        method: "GET".into(),
        url,
        headers: headers
            .iter()
            .map(|(name, value)| ((*name).into(), (*value).into()))
            .collect(),
        body: Vec::new(),
        signal: None,
    }
}

/// Whether `name --version` starts, whatever its exit status.
async fn command_exists(name: &str) -> bool {
    let mut command = maestro_process::command(name);
    command.arg("--version").kill_on_drop(true);
    let Ok(mut child) = CapturedChild::spawn(&mut command) else {
        return false;
    };
    wait_for_child_process(
        &mut child,
        WaitOptions::default(),
        std::future::pending(),
        None,
    )
    .await
    .is_ok()
}

/// Invocation-owned archive directory whose removal is attempted when dropped.
struct Staging {
    /// The directory.
    path: String,
}

impl Staging {
    /// Create a new uniquely named directory in `bin_dir`.
    fn create(bin_dir: &str, binary_name: &str) -> io::Result<Self> {
        let id = crate::output::output_accumulator::random_id()?;
        let name = format!("extract_tmp_{binary_name}_{}_{id}", std::process::id());
        let path = maestro_path::join(&[bin_dir, &name]);
        std::fs::create_dir(&path)?;
        Ok(Self { path })
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        // Cleanup failure never replaces the operation's own result.
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Extraction context preserving the original spawn failure.
#[derive(Debug)]
struct ExtractionError {
    /// Release asset being unpacked.
    asset: String,
    /// Native command error.
    source: io::Error,
}

impl std::fmt::Display for ExtractionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Failed to extract {}: {}",
            self.asset, self.source
        )
    }
}

impl std::error::Error for ExtractionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Unpack a gzip-compressed tar `archive` into `dir` with the `tar` command.
async fn extract_tar(asset: &str, archive: &str, dir: &str) -> io::Result<()> {
    let failed = |detail: &str| io::Error::other(format!("Failed to extract {asset}: {detail}"));
    let context = |source| {
        io::Error::other(ExtractionError {
            asset: asset.to_owned(),
            source,
        })
    };
    let mut command = maestro_process::command("tar");
    command.args(["xzf", archive, "-C", dir]).kill_on_drop(true);
    let mut child = CapturedChild::spawn(&mut command).map_err(context)?;
    let output = wait_for_child_process(
        &mut child,
        WaitOptions::default(),
        std::future::pending(),
        None,
    )
    .await
    .map_err(context)?;
    if output.status.success() {
        return Ok(());
    }
    Err(failed(
        String::from_utf8_lossy(&output.stderr).trim_matches(is_whitespace_scalar),
    ))
}

/// The unpacked program: the nested then the root candidate, else the first exact-name file
/// of a depth-first walk.
fn locate(dir: &str, stem: &str, binary: &str) -> io::Result<PathBuf> {
    let candidates = [
        maestro_path::join(&[dir, stem, binary]),
        maestro_path::join(&[dir, binary]),
    ];
    if let Some(found) = candidates
        .into_iter()
        .find(|candidate| Path::new(candidate).exists())
    {
        return Ok(PathBuf::from(found));
    }
    find_binary_recursively(Path::new(dir), binary)?.ok_or_else(|| {
        io::Error::other(format!(
            "Binary not found in archive: expected {binary} under {dir}"
        ))
    })
}

/// The first regular file named `file_name` below `root`; each directory's entries are visited
/// in name order and the last subdirectory found is searched first.
fn find_binary_recursively(root: &Path, file_name: &str) -> io::Result<Option<PathBuf>> {
    let mut stack = vec![root.to_owned()];
    while let Some(dir) = stack.pop() {
        for entry in maestro_listing::read_dir(&dir)? {
            let kind = entry.file_type()?;
            if kind.is_file() && entry.file_name() == file_name {
                return Ok(Some(entry.path()));
            }
            if kind.is_dir() {
                stack.push(entry.path());
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests;
