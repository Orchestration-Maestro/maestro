//! Native commands for repository development.

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::{Command, ExitCode};

/// Capture browser compilation diagnostics.
pub mod check_browser_build;
/// Copy prepared application assets.
pub mod copy_assets;
/// Format and verify captured index paths.
pub mod pre_commit;
/// Launch the application from its checkout.
pub mod run_source;
/// Run explicitly credential-free tests.
pub mod run_tests;

mod watch;

pub(crate) const CREDENTIALS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_OAUTH_TOKEN",
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "GROQ_API_KEY",
    "CEREBRAS_API_KEY",
    "XAI_API_KEY",
    "OPENROUTER_API_KEY",
    "ZAI_API_KEY",
    "MISTRAL_API_KEY",
    "MINIMAX_API_KEY",
    "MINIMAX_CN_API_KEY",
    "KIMI_API_KEY",
    "HF_TOKEN",
    "AI_GATEWAY_API_KEY",
    "OPENCODE_API_KEY",
    "COPILOT_GITHUB_TOKEN",
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "GOOGLE_CLOUD_PROJECT",
    "GCLOUD_PROJECT",
    "GOOGLE_CLOUD_LOCATION",
    "AWS_PROFILE",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "AWS_REGION",
    "AWS_DEFAULT_REGION",
    "AWS_BEARER_TOKEN_BEDROCK",
    "AWS_CONTAINER_CREDENTIALS_RELATIVE_URI",
    "AWS_CONTAINER_CREDENTIALS_FULL_URI",
    "AWS_WEB_IDENTITY_TOKEN_FILE",
    "FIREWORKS_API_KEY",
];

/// Dispatch a development command using the supplied Cargo executable and checkout.
///
/// # Errors
/// Returns native process and filesystem failures.
pub fn run(args: &[OsString], cargo: &Path, checkout: &Path) -> io::Result<ExitCode> {
    let (name, args) = args
        .split_first()
        .ok_or_else(|| io::Error::other("missing development command"))?;
    match name.to_str() {
        Some("asset-output") => asset_output(args, cargo),
        Some("clean-assets") => {
            for owner in ["app", "binary"] {
                let path = asset_directory(owner, cargo)?;
                match std::fs::remove_dir_all(path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Some("watch") => watch::run(args),
        Some("watch-step") => watch::step(args, cargo),
        Some("test-offline") => {
            let home =
                std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME is not set"))?;
            run_tests::run(Path::new(&home), Command::new("just").arg("test"))
        }
        Some("run-source") => Ok(run_source::run(
            cargo,
            checkout,
            args,
            run_source::Mode::Unix,
        )),
        Some("run-source-windows") => Ok(run_source::run(
            cargo,
            checkout,
            args,
            run_source::Mode::Windows,
        )),
        Some("check-browser-smoke") => browser_build(cargo, checkout),
        Some("pre-commit") => hook(cargo, checkout),
        Some("copy-assets") => {
            copy_assets::copy(
                argument(args, 0, "source")?,
                argument(args, 1, "destination")?,
                copy_assets::Layout::Library,
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Some("copy-binary-assets") => {
            let metadata = binary_manifest(cargo, checkout)?;
            copy_assets::copy(
                argument(args, 0, "source")?,
                argument(args, 1, "destination")?,
                copy_assets::Layout::Standalone {
                    metadata: &metadata,
                    viewer: argument(args, 2, "viewer")?,
                },
            )?;
            Ok(ExitCode::SUCCESS)
        }
        _ => Err(io::Error::other("unknown development command")),
    }
}

fn asset_output(args: &[OsString], cargo: &Path) -> io::Result<ExitCode> {
    let owner = args.first().and_then(|name| name.to_str()).unwrap_or("");
    println!("{}", asset_directory(owner, cargo)?.display());
    Ok(ExitCode::SUCCESS)
}

fn asset_directory(owner: &str, cargo: &Path) -> io::Result<std::path::PathBuf> {
    let directory = match owner {
        "app" => "maestro-app-assets",
        "binary" => "maestro-binary-assets",
        _ => return Err(io::Error::other("missing app or binary asset owner")),
    };
    let metadata = cargo_metadata::MetadataCommand::new()
        .cargo_path(cargo)
        .no_deps()
        .exec()
        .map_err(io::Error::other)?;
    Ok(metadata
        .target_directory
        .join(directory)
        .into_std_path_buf())
}

fn hook(cargo: &Path, checkout: &Path) -> io::Result<ExitCode> {
    let mut format = Command::new(cargo);
    format.current_dir(checkout).args(["fmt", "--all"]);
    let mut check = Command::new("just");
    check.current_dir(checkout).arg("check");
    let mut smoke = Command::new(std::env::current_exe()?);
    smoke.arg("check-browser-smoke");
    let active = checkout
        .join("crates/maestro-models/examples/browser_import_check.rs")
        .is_file();
    pre_commit::run(
        checkout,
        &mut format,
        &mut check,
        active.then_some(&mut smoke),
    )
}

fn browser_build(cargo: &Path, checkout: &Path) -> io::Result<ExitCode> {
    if !checkout
        .join("crates/maestro-models/examples/browser_import_check.rs")
        .is_file()
    {
        return Err(io::Error::other(
            "browser_import_check entry is not delivered",
        ));
    }
    let mut child = Command::new(cargo);
    child.current_dir(checkout).args([
        "build",
        "--locked",
        "-p",
        "maestro-models",
        "--example",
        "browser_import_check",
        "--target",
        "wasm32-unknown-unknown",
    ]);
    check_browser_build::run(
        &mut child,
        &std::env::temp_dir().join("maestro-browser-smoke-errors.log"),
    )
}

fn binary_manifest(cargo: &Path, checkout: &Path) -> io::Result<std::path::PathBuf> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .cargo_path(cargo)
        .manifest_path(checkout.join("Cargo.toml"))
        .no_deps()
        .other_options(vec!["--locked".into()])
        .exec()
        .map_err(io::Error::other)?;
    metadata
        .packages
        .into_iter()
        .find(|package| package.name.as_str() == "maestro")
        .map(|package| package.manifest_path.into_std_path_buf())
        .ok_or_else(|| io::Error::other("missing maestro binary package"))
}

fn argument<'a>(args: &'a [OsString], index: usize, name: &str) -> io::Result<&'a Path> {
    args.get(index)
        .map(Path::new)
        .ok_or_else(|| io::Error::other(format!("missing {name} parameter")))
}

pub(crate) fn exit_code(status: std::process::ExitStatus) -> ExitCode {
    ExitCode::from(
        status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .unwrap_or(1),
    )
}
