#[path = "repository_tools/cargo_directory.rs"]
mod cargo_directory;

use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
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

fn input(path: &Path, root: &Path, target: &Path, inputs: &Inputs) -> bool {
    if path.starts_with(target) || path.starts_with(root.join(".git")) {
        return false;
    }
    let shared = path.strip_prefix(root).is_ok_and(|relative| {
        matches!(
            relative.to_str(),
            Some("Cargo.toml" | "Cargo.lock" | "justfile" | "rust-toolchain.toml" | "mise.toml")
        ) || relative.starts_with(".cargo")
    });
    let selected = inputs
        .packages
        .iter()
        .filter(|(directory, _)| path.starts_with(directory))
        .max_by_key(|(directory, _)| directory.components().count())
        .is_some_and(|(_, selected)| *selected);
    if !shared && !selected {
        return false;
    }
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("rs" | "toml")
    ) || path
        .file_name()
        .is_some_and(|name| name == "Cargo.lock" || name == "justfile")
}

struct Inputs {
    packages: Vec<(PathBuf, bool)>,
}

fn resolve_inputs(
    cargo: &std::ffi::OsStr,
    root: &Path,
    selection: &[std::ffi::OsString],
) -> Result<Inputs, String> {
    let output = Command::new(cargo)
        .args(["metadata", "--format-version=1", "--locked"])
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or("Cargo packages missing")?;
    let mut selected = BTreeSet::new();
    if selection.is_empty() {
        for id in metadata["workspace_members"]
            .as_array()
            .ok_or("Cargo workspace members missing")?
        {
            selected.insert(id.as_str().ok_or("Cargo package ID missing")?.to_owned());
        }
    } else {
        for name in selection {
            let package = packages
                .iter()
                .find(|package| package["name"].as_str().is_some_and(|value| name == value))
                .ok_or_else(|| format!("unknown watch package: {}", name.to_string_lossy()))?;
            selected.insert(
                package["id"]
                    .as_str()
                    .ok_or("Cargo package ID missing")?
                    .to_owned(),
            );
        }
    }
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or("Cargo dependency graph missing")?;
    let mut pending = selected.iter().cloned().collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        let node = nodes
            .iter()
            .find(|node| node["id"].as_str() == Some(&id))
            .ok_or("Cargo dependency node missing")?;
        for dependency in node["dependencies"]
            .as_array()
            .ok_or("Cargo dependencies missing")?
        {
            let dependency = dependency.as_str().ok_or("Cargo dependency ID missing")?;
            if selected.insert(dependency.to_owned()) {
                pending.push(dependency.to_owned());
            }
        }
    }
    let mut inputs = Inputs {
        packages: Vec::new(),
    };
    for package in packages
        .iter()
        .filter(|package| package["source"].is_null())
    {
        let manifest = Path::new(
            package["manifest_path"]
                .as_str()
                .ok_or("Cargo manifest path missing")?,
        );
        inputs.packages.push((
            manifest
                .parent()
                .ok_or("Cargo package directory missing")?
                .to_owned(),
            selected.contains(package["id"].as_str().ok_or("Cargo package ID missing")?),
        ));
    }
    Ok(inputs)
}

fn run() -> Result<(), String> {
    let root = std::env::current_dir().map_err(|error| error.to_string())?;
    let mut args = std::env::args_os().skip(1);
    let cargo = args.next().unwrap_or_else(|| "cargo".into());
    let mut packages = Vec::new();
    while let Some(option) = args.next() {
        if option != "--package" {
            return Err("expected --package <name>".into());
        }
        packages.push(args.next().ok_or("expected package name")?);
    }
    let mut inputs = resolve_inputs(&cargo, &root, &packages)?;
    let mut selection = Vec::new();
    if packages.is_empty() {
        selection.push(std::ffi::OsString::from("--workspace"));
    } else {
        for package in &packages {
            selection.push("--package".into());
            selection.push(package.clone());
        }
    }
    let mut target =
        cargo_directory::resolve(&cargo, &root, &std::env::vars_os().collect::<Vec<_>>())?;
    let (send, receive) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
        let _ = send.send(event);
    })
    .map_err(|error| error.to_string())?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|error| error.to_string())?;
    let mut registered = BTreeSet::new();
    register(&mut watcher, &root, &inputs, &mut registered)?;
    loop {
        let status = Command::new(&cargo)
            .args(["build", "--locked"])
            .args(&selection)
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
                && event.paths.iter().any(|path| {
                    !path.starts_with(&target)
                        && !path.starts_with(root.join(".git"))
                        && (input(path, &root, &target, &inputs)
                            || path.file_name().is_some_and(|name| name == "Cargo.toml")
                            || path == &root.join(".cargo")
                            || (matches!(
                                event.kind,
                                EventKind::Create(notify::event::CreateKind::Folder)
                                    | EventKind::Remove(notify::event::RemoveKind::Folder)
                            ) && (path.join("Cargo.toml").is_file()
                                || inputs
                                    .packages
                                    .iter()
                                    .any(|(directory, _)| directory == path))))
                })
            {
                // Coalesce events already queued, retaining changes during the next build.
                while let Ok(event) = receive.try_recv() {
                    event.map_err(|error| error.to_string())?;
                }
                let refreshed = resolve_inputs(&cargo, &root, &packages).and_then(|inputs| {
                    cargo_directory::resolve(
                        &cargo,
                        &root,
                        &std::env::vars_os().collect::<Vec<_>>(),
                    )
                    .map(|target| (inputs, target))
                });
                match refreshed {
                    Ok((next_inputs, next_target)) => {
                        register(&mut watcher, &root, &next_inputs, &mut registered)?;
                        inputs = next_inputs;
                        target = next_target;
                    }
                    Err(error) => eprintln!("developer watch: {error}"),
                }
                break;
            }
        }
    }
}

fn register(
    watcher: &mut impl Watcher,
    root: &Path,
    inputs: &Inputs,
    registered: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    let next = inputs
        .packages
        .iter()
        .filter(|(directory, selected)| *selected && !directory.starts_with(root))
        .map(|(directory, _)| directory.clone())
        .collect::<BTreeSet<_>>();
    for directory in registered.difference(&next) {
        watcher
            .unwatch(directory)
            .map_err(|error| error.to_string())?;
    }
    for directory in next.difference(registered) {
        watcher
            .watch(directory, RecursiveMode::Recursive)
            .map_err(|error| error.to_string())?;
    }
    *registered = next;
    Ok(())
}
