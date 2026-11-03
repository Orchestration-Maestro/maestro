//! Release selection, downloads, archives and advice on chosen hosts.
#![cfg(test)]

#[cfg(unix)]
mod archive_security;
#[path = "../../../tests/support/runtime.rs"]
mod runtime;

use super::*;
use runtime::{run, run_paused};
use serde::Deserialize;
use serde_json::Value;

/// A recorded asset selection.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetCase {
    /// Selected program.
    tool: String,
    /// Host operating system.
    os: String,
    /// Host architecture.
    arch: String,
    /// Release version.
    version: String,
    /// Expected filename.
    asset: Option<String>,
}

/// Named recorded groups; unknown groups are rejected.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    /// Recorded release assets follow tool platform and architecture cases.
    release_assets_follow_tool_platform_and_architecture: Vec<Value>,
    /// Recorded release lookup strips one lowercase v cases.
    release_lookup_strips_one_lowercase_v: Vec<Value>,
    /// Recorded release lookup reads the last tag name text cases.
    release_lookup_reads_the_last_tag_name_text: Vec<Value>,
    /// Recorded release lookup accepts only success statuses cases.
    release_lookup_accepts_only_success_statuses: Vec<Value>,
    /// Recorded recursive search prefers files then the last directory cases.
    recursive_search_prefers_files_then_the_last_directory: Vec<Value>,
    /// Recorded windows binary directory entries carry the exe suffix cases.
    windows_binary_directory_entries_carry_the_exe_suffix: Vec<Value>,
    /// Recorded missing programs get platform advice or a lookup failure cases.
    missing_programs_get_platform_advice_or_a_lookup_failure: Vec<Value>,
    /// Recorded archive downloads accept only success statuses cases.
    archive_downloads_accept_only_success_statuses: Vec<Value>,
    /// Recorded linux archives publish an executable binary cases.
    linux_archives_publish_an_executable_binary: Vec<Value>,
    /// Recorded windows archives publish the exe unchanged cases.
    windows_archives_publish_the_exe_unchanged: Vec<Value>,
    /// Recorded failed extractions publish nothing cases.
    failed_extractions_publish_nothing: Vec<Value>,
    /// Recorded failed download leaves no archive behind cases.
    failed_download_leaves_no_archive_behind: Vec<Value>,
    /// Recorded user file named like the asset survives an install cases.
    user_file_named_like_the_asset_survives_an_install: Vec<Value>,
    /// Recorded progress styles follow the color flag cases.
    progress_styles_follow_the_color_flag: Vec<Value>,
}

/// Decode one named group of recorded cases.
fn cases<T: serde::de::DeserializeOwned>(name: &str) -> Vec<T> {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../tests/fixtures/search_tool_cases.json"
    ))
    .unwrap();
    let rows = match name {
        "release_assets_follow_tool_platform_and_architecture" => {
            fixture.release_assets_follow_tool_platform_and_architecture
        }
        "release_lookup_strips_one_lowercase_v" => fixture.release_lookup_strips_one_lowercase_v,
        "release_lookup_reads_the_last_tag_name_text" => {
            fixture.release_lookup_reads_the_last_tag_name_text
        }
        "release_lookup_accepts_only_success_statuses" => {
            fixture.release_lookup_accepts_only_success_statuses
        }
        "recursive_search_prefers_files_then_the_last_directory" => {
            fixture.recursive_search_prefers_files_then_the_last_directory
        }
        "windows_binary_directory_entries_carry_the_exe_suffix" => {
            fixture.windows_binary_directory_entries_carry_the_exe_suffix
        }
        "missing_programs_get_platform_advice_or_a_lookup_failure" => {
            fixture.missing_programs_get_platform_advice_or_a_lookup_failure
        }
        "archive_downloads_accept_only_success_statuses" => {
            fixture.archive_downloads_accept_only_success_statuses
        }
        "linux_archives_publish_an_executable_binary" => {
            fixture.linux_archives_publish_an_executable_binary
        }
        "windows_archives_publish_the_exe_unchanged" => {
            fixture.windows_archives_publish_the_exe_unchanged
        }
        "failed_extractions_publish_nothing" => fixture.failed_extractions_publish_nothing,
        "failed_download_leaves_no_archive_behind" => {
            fixture.failed_download_leaves_no_archive_behind
        }
        "user_file_named_like_the_asset_survives_an_install" => {
            fixture.user_file_named_like_the_asset_survives_an_install
        }
        "progress_styles_follow_the_color_flag" => fixture.progress_styles_follow_the_color_flag,
        _ => panic!("unknown fixture group"),
    };
    serde_json::from_value(Value::Array(rows)).unwrap()
}

/// Select the recorded program.
fn tool(name: &str) -> SearchTool {
    match name {
        "fd" => SearchTool::Fd,
        "rg" => SearchTool::Rg,
        _ => panic!("unknown fixture program"),
    }
}

#[test]
fn release_assets_follow_tool_platform_and_architecture() {
    for row in cases::<AssetCase>("release_assets_follow_tool_platform_and_architecture") {
        let asset = tool(&row.tool).asset(
            &row.version,
            Host {
                os: host_name(&row.os),
                arch: host_name(&row.arch),
            },
        );
        assert_eq!(asset.map(|asset| asset.file_name()), row.asset);
    }
}

/// Intern the closed host vocabulary in the recorded fixture.
fn host_name(name: &str) -> &'static str {
    match name {
        "macos" => "macos",
        "linux" => "linux",
        "windows" => "windows",
        "freebsd" => "freebsd",
        "android" => "android",
        "aarch64" => "aarch64",
        "x86_64" => "x86_64",
        "x86" => "x86",
        _ => panic!("unknown fixture host"),
    }
}

use futures_util::stream;
use maestro_models::{Fetch, HttpResponse};
use std::sync::{Arc, Mutex};

/// A selected release version or failure.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseCase {
    /// Response status.
    status: u16,
    /// Raw response body.
    body: String,
    /// Expected version.
    version: Option<String>,
    /// Expected authored failure, if recorded.
    failure: Option<String>,
}

/// A response holding one complete body chunk.
fn response(status: u16, bytes: Vec<u8>) -> HttpResponse {
    HttpResponse {
        status,
        status_text: String::new(),
        headers: std::collections::BTreeMap::default(),
        body: Box::pin(stream::iter([Ok(bytes)])),
    }
}

/// Exercise a release selection group through a recorded transport.
async fn releases(name: &str) {
    for row in cases::<ReleaseCase>(name) {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&requests);
        let fetch: Fetch = Arc::new(move |request| {
            seen.lock().unwrap().push(request);
            let answer = response(row.status, row.body.as_bytes().to_vec());
            Box::pin(async { Ok(answer) })
        });
        let manager = ToolsManager::new(String::new(), Some(fetch));
        let result = manager.get_latest_version("sharkdp/fd").await;
        if let Some(version) = row.version {
            assert_eq!(result.unwrap(), version);
        } else {
            let error = result.unwrap_err();
            if let Some(failure) = row.failure {
                assert_eq!(error.to_string(), failure);
            }
        }
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(
            request.url,
            "https://api.github.com/repos/sharkdp/fd/releases/latest"
        );
        assert_eq!(request.method, "GET");
        assert_eq!(
            request.headers.get("user-agent").unwrap(),
            "maestro-coding-agent"
        );
        assert!(request.body.is_empty());
        assert!(request.signal.is_none());
    }
}

#[test]
fn release_lookup_strips_one_lowercase_v() {
    run(async {
        releases("release_lookup_strips_one_lowercase_v").await;
    });
}

#[test]
fn release_lookup_reads_the_last_tag_name_text() {
    run(async {
        releases("release_lookup_reads_the_last_tag_name_text").await;
    });
}

#[test]
fn release_lookup_accepts_only_success_statuses() {
    run(async {
        releases("release_lookup_accepts_only_success_statuses").await;
    });
}

/// A download response with exactly one recorded body source.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Download {
    /// Response status.
    status: u16,
    /// Archive fixture name.
    archive: Option<String>,
    /// Raw body text.
    text: Option<String>,
    /// First chunk before a connection failure.
    broken_after: Option<String>,
}

/// The published binary observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Binary {
    /// Expected text.
    contents: String,
    /// Expected Unix permission bits.
    mode: u32,
}

/// A recorded request observation.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    /// URL.
    url: String,
    /// Optional user agent.
    user_agent: Option<String>,
}

/// A complete installation observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Install {
    /// Case label.
    id: String,
    /// Selected program.
    tool: String,
    /// Selected host.
    os: String,
    /// Suppress output.
    silent: bool,
    /// Enable colour.
    color: bool,
    /// Release tag.
    tag: String,
    /// Lookup status.
    api_status: u16,
    /// Archive response.
    download: Option<Download>,
    /// User-owned archive-named file.
    preexisting: Option<String>,
    /// Initial bin directory presence.
    bin_exists: bool,
    /// Expected path.
    result: Option<String>,
    /// Published content and mode.
    binary: Option<Binary>,
    /// Final directory entries, or absence.
    remaining: Option<Vec<String>>,
    /// Complete output lines before any native diagnostic.
    lines: Vec<String>,
    /// Prefix of a final native diagnostic.
    last_line_prefix: Option<String>,
    /// Ordered requests.
    requests: Vec<Request>,
}

/// Read a committed archive.
fn archive(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/search_tool_archives")
            .join(name),
    )
    .unwrap()
}

/// Answer a recorded download.
fn download_response(row: Download) -> HttpResponse {
    let mut sources = 0;
    let mut chunks = Vec::new();
    if let Some(name) = row.archive {
        sources += 1;
        chunks.push(Ok(archive(&name)));
    }
    if let Some(text) = row.text {
        sources += 1;
        chunks.push(Ok(text.into_bytes()));
    }
    if let Some(text) = row.broken_after {
        sources += 1;
        chunks.push(Ok(text.into_bytes()));
        chunks.push(Err(FetchError::Connection(
            maestro_models::DiagnosticErrorInfo {
                message: "controlled body failure".into(),
                name: None,
                stack: None,
                code: None,
            },
        )));
    }
    assert_eq!(sources, 1);
    let mut answer = response(row.status, Vec::new());
    answer.body = Box::pin(stream::iter(chunks));
    answer
}

/// Transport of an installation with ordered request recording.
fn install_fetch(row: &Install, requests: &Arc<Mutex<Vec<Request>>>) -> Fetch {
    let seen = Arc::clone(requests);
    let (status, tag, download) = (row.api_status, row.tag.clone(), row.download.clone());
    Arc::new(move |request| {
        assert_eq!(request.method, "GET");
        assert!(request.body.is_empty());
        assert!(request.signal.is_none());
        let lookup = request.url.starts_with("https://api.github.com/");
        seen.lock().unwrap().push(Request {
            url: request.url,
            user_agent: request.headers.get("user-agent").cloned(),
        });
        let answer = if lookup {
            response(
                status,
                serde_json::to_vec(&serde_json::json!({"tag_name": tag})).unwrap(),
            )
        } else {
            download_response(download.clone().unwrap())
        };
        Box::pin(async { Ok(answer) })
    })
}

/// Expand only the recorded path and process placeholders.
fn expand(text: &str, bin: &str) -> String {
    // The installer joins below the directory natively; the recording joins with `/`.
    text.replace("<BIN>/", &format!("{bin}{}", std::path::MAIN_SEPARATOR))
        .replace("<BIN>", bin)
        .replace("<PID>", &std::process::id().to_string())
}

/// Check the completed output without discarding multiline diagnostics.
fn output_matches(row: &Install, output: &[u8], bin: &str) {
    let output = String::from_utf8(output.to_vec()).unwrap();
    let mut prefix = row
        .lines
        .iter()
        .map(|line| expand(line, bin))
        .collect::<Vec<_>>()
        .join("\n");
    if !row.lines.is_empty() {
        prefix.push('\n');
    }
    if let Some(last) = &row.last_line_prefix {
        let expected = format!("{prefix}{}", expand(last, bin));
        assert!(
            output.starts_with(&expected),
            "{} output prefix: {output:?}",
            row.id
        );
        let detail = output
            .strip_prefix(&expected)
            .unwrap()
            .strip_suffix('\n')
            .unwrap();
        assert!(!detail.is_empty(), "{} diagnostic detail", row.id);
        assert!(
            !detail.ends_with(is_whitespace_scalar),
            "{} diagnostic trim",
            row.id
        );
        if row.color {
            assert!(detail.ends_with("\x1b[39m"));
        }
    } else {
        assert_eq!(output, prefix, "{} output", row.id);
    }
}

/// Final inventory, binary and user-file observations after the operation finishes.
fn installed_matches(row: &Install, bin: &str, result: Option<&String>) {
    assert_eq!(
        result,
        row.result.as_ref().map(|path| expand(path, bin)).as_ref(),
        "{} result",
        row.id
    );
    let entries = std::fs::read_dir(bin).ok().map(|entries| {
        let mut names: Vec<_> = entries
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    });
    assert_eq!(entries, row.remaining, "{} inventory", row.id);
    if let Some(binary) = &row.binary {
        let path = row.result.as_ref().map(|path| expand(path, bin)).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), binary.contents);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                binary.mode
            );
        }
        #[cfg(not(unix))]
        let _ = binary.mode;
    }
    if let Some(name) = &row.preexisting {
        assert_eq!(
            std::fs::read_to_string(Path::new(bin).join(name)).unwrap(),
            "user file"
        );
    }
    assert!(!Path::new(bin).join("outside").exists());
    assert!(!Path::new(bin).parent().unwrap().join("outside").exists());
}

/// Runs the calling test again in a child without `MAESTRO_OFFLINE`, so an offline flag in
/// the developer's shell never reaches the install. True in that child; false in the caller
/// once the child passed.
fn online(test: &str) -> bool {
    if maestro_test_process::selection().is_child() {
        return true;
    }
    let child = maestro_test_process::TestProcess::current(test).unwrap();
    let output = child
        .case("*")
        .env_remove("MAESTRO_OFFLINE")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    false
}

/// Whether the host `tar` extracts an empty file as an archive, probed inside `dir`.
fn accepts_empty_archive(dir: &str) -> bool {
    let probe = Path::new(dir).join("empty.tar.gz");
    std::fs::write(&probe, b"").unwrap();
    let status = std::process::Command::new("tar")
        .arg("xzf")
        .arg(&probe)
        .args(["-C", dir])
        .status()
        .unwrap();
    std::fs::remove_file(probe).unwrap();
    status.success()
}

/// Run the named installation group on its chosen hosts.
async fn installs(name: &str) {
    if !online(name) {
        return;
    }
    for row in cases::<Install>(name) {
        let scratch = Staging::create(std::env::temp_dir().to_str().unwrap(), "unit").unwrap();
        // A host `tar` that accepts an empty archive reaches the binary check instead of the
        // recorded extraction failure.
        let empty = (row.download.as_ref()).is_some_and(|body| body.text.as_deref() == Some(""));
        if empty && accepts_empty_archive(&scratch.path) {
            eprintln!("{} needs a host tar that refuses an empty archive", row.id);
            continue;
        }
        let bin = maestro_path::join(&[
            &scratch.path,
            if row.bin_exists { "bin" } else { "nested/bin" },
        ]);
        if row.bin_exists {
            std::fs::create_dir(&bin).unwrap();
        }
        if let Some(name) = &row.preexisting {
            std::fs::write(Path::new(&bin).join(name), "user file").unwrap();
        }
        let requests = Arc::new(Mutex::new(Vec::new()));
        let mut manager = ToolsManager::new(bin.clone(), Some(install_fetch(&row, &requests)));
        manager.host = Host {
            os: host_name(&row.os),
            arch: "x86_64",
        };
        let mut output = Vec::new();
        let writer = (!row.silent).then_some(ToolsManagerOutput {
            stdout: &mut output,
            color: row.color,
        });
        let result = manager.install_missing(tool(&row.tool), writer).await;
        output_matches(&row, &output, &bin);
        assert_eq!(
            *requests.lock().unwrap(),
            row.requests,
            "{} requests",
            row.id
        );
        installed_matches(&row, &bin, result.as_ref());
    }
}

#[test]
fn linux_archives_publish_an_executable_binary() {
    run(async {
        installs("linux_archives_publish_an_executable_binary").await;
    });
}

#[test]
fn user_file_named_like_the_asset_survives_an_install() {
    run(async {
        installs("user_file_named_like_the_asset_survives_an_install").await;
    });
}

#[test]
fn windows_archives_publish_the_exe_unchanged() {
    run(async {
        installs("windows_archives_publish_the_exe_unchanged").await;
    });
}

#[test]
fn missing_programs_get_platform_advice_or_a_lookup_failure() {
    run(async {
        installs("missing_programs_get_platform_advice_or_a_lookup_failure").await;
    });
}

#[test]
fn archive_downloads_accept_only_success_statuses() {
    run(async {
        installs("archive_downloads_accept_only_success_statuses").await;
    });
}

#[test]
fn failed_extractions_publish_nothing() {
    run(async {
        installs("failed_extractions_publish_nothing").await;
    });
}

#[test]
fn failed_download_leaves_no_archive_behind() {
    run(async {
        installs("failed_download_leaves_no_archive_behind").await;
    });
}

#[test]
fn progress_styles_follow_the_color_flag() {
    run(async {
        installs("progress_styles_follow_the_color_flag").await;
    });
}

/// A tree whose exact-name walk has a recorded result.
#[cfg(unix)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WalkCase {
    /// Regular files.
    files: Vec<String>,
    /// Symlink name and target pairs.
    links: Vec<(String, String)>,
    /// Selected relative path.
    found: Option<String>,
}

#[cfg(unix)]
#[test]
fn recursive_search_prefers_files_then_the_last_directory() {
    for row in cases::<WalkCase>("recursive_search_prefers_files_then_the_last_directory") {
        let scratch = Staging::create(std::env::temp_dir().to_str().unwrap(), "walk").unwrap();
        let root = Path::new(&scratch.path);
        for file in row.files {
            let path = root.join(&file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, file).unwrap();
        }
        for (name, target) in row.links {
            std::os::unix::fs::symlink(target, root.join(name)).unwrap();
        }
        let result = find_binary_recursively(root, "fd").unwrap();
        assert_eq!(
            result.map(|path| path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned()),
            row.found
        );
    }
}

/// A local Windows candidate.
#[cfg(unix)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryCase {
    /// Entry kind.
    kind: String,
    /// Joined result.
    found: String,
}

#[cfg(unix)]
#[test]
fn windows_binary_directory_entries_carry_the_exe_suffix() {
    run(async {
        for row in cases::<EntryCase>("windows_binary_directory_entries_carry_the_exe_suffix") {
            let scratch = Staging::create(std::env::temp_dir().to_str().unwrap(), "entry").unwrap();
            let path = Path::new(&scratch.path).join("fd.exe");
            match row.kind.as_str() {
                "file" => std::fs::write(&path, "not executable").unwrap(),
                "directory" => std::fs::create_dir(&path).unwrap(),
                "symlink" => {
                    std::fs::write(Path::new(&scratch.path).join("target"), "target").unwrap();
                    std::os::unix::fs::symlink("target", &path).unwrap();
                }
                _ => panic!("unknown fixture entry"),
            }
            let mut manager = ToolsManager::new(scratch.path.clone(), None);
            manager.host = Host {
                os: "windows",
                arch: "x86_64",
            };
            assert_eq!(
                manager.get_tool_path(SearchTool::Fd).await,
                Some(expand(&row.found, &scratch.path))
            );
        }
    });
}

/// Fd stops at the last Intel macOS build; every other host asks for the latest release.
#[test]
fn fd_downloads_pin_to_the_last_intel_macos_build() {
    const ASSET: &str = "https://github.com/sharkdp/fd/releases/download";
    const LOOKUP: &str = "https://api.github.com/repos/sharkdp/fd/releases/latest";
    let cases = [
        (
            "macos",
            "x86_64",
            vec![format!(
                "{ASSET}/v10.3.0/fd-v10.3.0-x86_64-apple-darwin.tar.gz"
            )],
        ),
        (
            "macos",
            "aarch64",
            vec![
                LOOKUP.to_owned(),
                format!("{ASSET}/v11.0.0/fd-v11.0.0-aarch64-apple-darwin.tar.gz"),
            ],
        ),
        (
            "linux",
            "x86_64",
            vec![
                LOOKUP.to_owned(),
                format!("{ASSET}/v11.0.0/fd-v11.0.0-x86_64-unknown-linux-gnu.tar.gz"),
            ],
        ),
    ];
    for (os, arch, expected) in cases {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let bin = maestro_path::join(&[scratch.path().to_str().unwrap(), "bin"]);
        let urls = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&urls);
        let fetch: Fetch = Arc::new(move |request| {
            let answer = if request.url == LOOKUP {
                response(200, br#"{"tag_name":"v11.0.0"}"#.to_vec())
            } else {
                response(404, Vec::new())
            };
            seen.lock().unwrap().push(request.url);
            Box::pin(async { Ok(answer) })
        });
        let mut manager = ToolsManager::new(bin, Some(fetch));
        manager.host = Host { os, arch };
        assert_eq!(run(manager.install_missing(SearchTool::Fd, None)), None);
        assert_eq!(*urls.lock().unwrap(), expected, "{os} {arch}");
    }
}

#[test]
fn lookup_and_download_deadlines_cover_the_whole_body() {
    if !online("lookup_and_download_deadlines_cover_the_whole_body") {
        return;
    }
    run_paused(async {
        for phase in 0..3 {
            let scratch =
                Staging::create(std::env::temp_dir().to_str().unwrap(), "deadline").unwrap();
            let bin = maestro_path::join(&[&scratch.path, "bin"]);
            let fetch = stalled_fetch(phase);
            let mut manager = ToolsManager::new(bin.clone(), Some(fetch));
            manager.host = Host {
                os: "linux",
                arch: "x86_64",
            };
            let mut output = Vec::new();
            let started = tokio::time::Instant::now();
            assert_eq!(
                manager
                    .install_missing(
                        SearchTool::Fd,
                        Some(ToolsManagerOutput {
                            stdout: &mut output,
                            color: false
                        })
                    )
                    .await,
                None
            );
            assert_eq!(
                started.elapsed().as_millis(),
                if phase == 2 { 120_000 } else { 10_000 }
            );
            assert_eq!(
                String::from_utf8(output).unwrap(),
                "fd not found. Downloading...\nFailed to download fd: Request timed out.\n"
            );
            if phase == 2 {
                assert_eq!(std::fs::read_dir(&bin).unwrap().count(), 0);
            } else {
                assert!(!Path::new(&bin).exists());
            }
        }
    });
}

#[test]
fn concurrent_installs_stage_in_separate_directories() {
    if !online("concurrent_installs_stage_in_separate_directories") {
        return;
    }
    run(async {
        let scratch =
            Staging::create(std::env::temp_dir().to_str().unwrap(), "concurrent").unwrap();
        let bin = maestro_path::join(&[&scratch.path, "bin"]);
        let fetch = concurrent_fetch(bin.clone());
        let mut manager = ToolsManager::new(bin.clone(), Some(fetch));
        manager.host = Host {
            os: "linux",
            arch: "x86_64",
        };
        let (mut first, mut second) = (Vec::new(), Vec::new());
        let results = tokio::join!(
            manager.install_missing(
                SearchTool::Fd,
                Some(ToolsManagerOutput {
                    stdout: &mut first,
                    color: false
                })
            ),
            manager.install_missing(
                SearchTool::Fd,
                Some(ToolsManagerOutput {
                    stdout: &mut second,
                    color: false
                })
            )
        );
        assert_eq!(results, (None, None));
        let prefix = format!(
            "Failed to download fd: Binary not found in archive: expected fd under {bin}{}extract_tmp_fd_{}_",
            std::path::MAIN_SEPARATOR,
            std::process::id()
        );
        let messages: Vec<_> = [first, second]
            .into_iter()
            .map(|bytes| {
                let text = String::from_utf8(bytes).unwrap();
                let line = text.lines().last().unwrap();
                assert!(line.starts_with(&prefix));
                line.to_owned()
            })
            .collect();
        assert_ne!(messages[0], messages[1], "staging names must differ");
        assert_eq!(std::fs::read_dir(bin).unwrap().count(), 0);
    });
}

/// Transport stalling headers or the body at the selected phase.
fn stalled_fetch(phase: usize) -> Fetch {
    Arc::new(move |request| {
        Box::pin(async move {
            let lookup = request.url.starts_with("https://api.github.com/");
            if phase == 0 {
                return std::future::pending().await;
            }
            if phase == 2 && lookup {
                return Ok(response(200, br#"{"tag_name":"v1"}"#.to_vec()));
            }
            let mut answer = response(200, Vec::new());
            answer.body = if phase == 1 {
                Box::pin(stream::pending())
            } else {
                use futures_util::StreamExt;
                Box::pin(stream::iter([Ok(b"partial".to_vec())]).chain(stream::pending()))
            };
            Ok(answer)
        })
    })
}

/// Hold both downloads while their distinct staging directories are observed.
fn concurrent_fetch(stage_root: String) -> Fetch {
    let barrier = Arc::new(tokio::sync::Barrier::new(2));

    Arc::new(move |request| {
        let stage_root = stage_root.clone();
        let barrier = Arc::clone(&barrier);
        Box::pin(async move {
            if request.url.starts_with("https://api.github.com/") {
                return Ok(response(200, br#"{"tag_name":"v1.0.0"}"#.to_vec()));
            }
            barrier.wait().await;
            let names: Vec<_> = std::fs::read_dir(&stage_root)
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect();
            assert_eq!(
                names.len(),
                2,
                "both paused installs have distinct staging directories"
            );
            assert_ne!(names[0], names[1]);
            barrier.wait().await;
            Ok(response(200, archive("linux-missing.tar.gz")))
        })
    })
}
