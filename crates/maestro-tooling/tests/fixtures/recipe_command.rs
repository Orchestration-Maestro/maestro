//! Command fixture for exercising repository recipes with controlled tools.

use std::{
    env, fs,
    io::Write,
    path::Path,
    process::Command,
};
/// Routes fixture calls to their controlled or native command.
fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("run")
        && env::var_os("MAESTRO_REAL_CARGO").is_some()
    {
        if let Some(binary) = env::var_os("MAESTRO_DEVELOPMENT") {
            let start = args.iter().position(|arg| arg == "--").unwrap() + 1;
            exit_command(Command::new(binary).args(&args[start..]));
        }
    }
    if Path::new(&env::args().next().unwrap()).file_stem().unwrap() == "watchexec" {
        if let Some(watcher) = env::var_os("MAESTRO_REAL_WATCHEXEC") {
            exit_command(
                Command::new(watcher)
                    .args(["--wrap-process=none"])
                    .args(&args),
            );
        }
    }
    if let Some(cargo) = env::var_os("MAESTRO_REAL_CARGO") {
        exit_command(Command::new(cargo).args(&args));
    }
    log_command(&args);
    if args.first().map(String::as_str) == Some("run") {
        if let Some(binary) = env::var_os("MAESTRO_DEVELOPMENT") {
            let start = args.iter().position(|arg| arg == "--").unwrap() + 1;
            exit_command(Command::new(binary).args(&args[start..]));
        }
    }
    if env::var("MAESTRO_FAIL").ok().as_deref() == args.first().map(String::as_str) {
        std::process::exit(17);
    }
}

/// Records fake command arguments and the requested environment observations.
fn log_command(args: &[String]) {
    if let Some(path) = env::var_os("MAESTRO_CWD") {
        fs::write(
            path,
            env::current_dir().unwrap().as_os_str().as_encoded_bytes(),
        )
        .unwrap();
    }
    let name = Path::new(&env::args().next().unwrap())
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let mut log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(env::var_os("MAESTRO_LOG").unwrap())
        .unwrap();
    writeln!(log, "{name} {}", args.join("|")).unwrap();
    if let Some(path) = env::var_os("MAESTRO_ENV_RESULT") {
        let auth = Path::new(&env::var_os("HOME").unwrap()).join(".maestro/agent/auth.json");
        fs::write(
            path,
            format!(
                "key={} auth={} flag={}\n",
                env::var_os("OPENAI_API_KEY").is_some(),
                auth.is_file(),
                env::var("MAESTRO_NO_LOCAL_LLM").unwrap_or_default()
            ),
        )
        .unwrap();
    }
}

/// Returns the native command status to the recipe caller.
fn exit_command(command: &mut Command) -> ! {
    std::process::exit(command.status().unwrap().code().unwrap());
}
