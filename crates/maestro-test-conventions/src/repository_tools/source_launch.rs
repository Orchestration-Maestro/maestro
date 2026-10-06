use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

const REMOVED: &[&str] = &[
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
    "AZURE_OPENAI_API_KEY",
    "AZURE_OPENAI_BASE_URL",
    "AZURE_OPENAI_RESOURCE_NAME",
];

pub(super) fn run(shell: &str, root: &Path, args: Vec<OsString>) -> Result<u8, String> {
    if !matches!(shell, "bash" | "powershell") {
        return Err("repository tools: expected bash or powershell source adapter".into());
    }
    let mut no_env = false;
    let args = args
        .into_iter()
        .filter(|arg| {
            let switch = if shell == "powershell" {
                arg.to_str()
                    .is_some_and(|text| text.eq_ignore_ascii_case("--no-env"))
            } else {
                arg == "--no-env"
            };
            no_env |= switch;
            !switch
        })
        .collect::<Vec<_>>();
    if no_env {
        println!("Running without API keys...");
    }
    let cargo = if let Some(cargo) = std::env::var_os("CARGO") {
        cargo
    } else {
        let output = Command::new("rustup")
            .args(["which", "cargo"])
            .current_dir(root)
            .output()
            .map_err(|error| format!("repository tools: resolve cargo: {error}"))?;
        if !output.status.success() {
            return Ok(super::status_code(output.status));
        }
        OsString::from(
            String::from_utf8(output.stdout)
                .map_err(|error| error.to_string())?
                .trim_end_matches(['\r', '\n']),
        )
    };
    if !available(shell, Path::new(&cargo)) {
        eprintln!(
            "cargo not found at {}. Run just setup from the repo root first.",
            Path::new(&cargo).display()
        );
        return Ok(1);
    }
    let configure = |command: &mut Command| {
        if no_env {
            for name in REMOVED {
                command.env_remove(name);
            }
            if shell == "bash" {
                command.env_remove("HF_TOKEN");
            }
        }
    };
    let mut build = Command::new(&cargo);
    build
        .args(["build", "-p", "maestro", "--locked"])
        .current_dir(root);
    configure(&mut build);
    let status = build
        .status()
        .map_err(|error| format!("repository tools: build source: {error}"))?;
    if !status.success() {
        return Ok(super::status_code(status));
    }
    let target = super::cargo_directory::resolve(&cargo, root)?;
    let mut application = Command::new(
        target
            .join("debug")
            .join(format!("maestro{}", std::env::consts::EXE_SUFFIX)),
    );
    application.args(args);
    configure(&mut application);
    application
        .status()
        .map(super::status_code)
        .map_err(|error| format!("repository tools: launch source: {error}"))
}

fn available(shell: &str, path: &Path) -> bool {
    #[cfg(unix)]
    if shell == "bash" {
        use std::os::unix::ffi::OsStrExt;
        unsafe extern "C" {
            fn access(path: *const std::ffi::c_char, mode: i32) -> i32;
        }
        let Ok(path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
            return false;
        };
        // Match the shell's executable-access check, including directory search permissions.
        return unsafe { access(path.as_ptr(), 1) == 0 };
    }
    #[cfg(not(unix))]
    let _ = shell;
    path.is_file()
}
