use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::Path;
use std::process::{Command, ExitCode};

pub(super) fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("developer watch: {error}");
            ExitCode::FAILURE
        }
    }
}

fn input(path: &Path, root: &Path, target: &Path) -> bool {
    if path.starts_with(target) || path.starts_with(root.join(".git")) {
        return false;
    }
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("rs" | "toml")
    ) || path
        .file_name()
        .is_some_and(|name| name == "Cargo.lock" || name == "justfile")
}

fn run() -> Result<(), String> {
    let root = std::env::current_dir().map_err(|error| error.to_string())?;
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                root.join(path)
            }
        })
        .unwrap_or_else(|| root.join("target"));
    let cargo = std::env::args_os().nth(1).unwrap_or_else(|| "cargo".into());
    let (send, receive) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
        let _ = send.send(event);
    })
    .map_err(|error| error.to_string())?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|error| error.to_string())?;
    loop {
        let status = Command::new(&cargo)
            .args(["build", "--workspace", "--locked"])
            .status()
            .map_err(|error| error.to_string())?;
        if !status.success() {
            eprintln!("developer watch: build failed: {status}");
        }
        loop {
            let event = receive
                .recv()
                .map_err(|error| error.to_string())?
                .map_err(|error| error.to_string())?;
            if !matches!(event.kind, EventKind::Access(_))
                && event.paths.iter().any(|path| input(path, &root, &target))
            {
                // Coalesce events already queued, retaining changes during the next build.
                while let Ok(event) = receive.try_recv() {
                    event.map_err(|error| error.to_string())?;
                }
                break;
            }
        }
    }
}
