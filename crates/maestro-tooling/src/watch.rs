use std::ffi::OsString;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde::Deserialize;

pub(crate) fn run(args: &[OsString]) -> io::Result<ExitCode> {
    let status = Command::new("watchexec")
        .args([
            "--watch",
            ".",
            "--ignore-nothing",
            "--ignore",
            "/.git/",
            "--on-busy-update=queue",
            "--shell=none",
            "--emit-events-to=json-stdio",
            "--",
        ])
        .arg(std::env::current_exe()?)
        .args(["watch-step", "--"])
        .args(args)
        .status()?;
    Ok(super::exit_code(status))
}

pub(crate) fn step(args: &[OsString], cargo: &Path) -> io::Result<ExitCode> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .cargo_path(cargo)
        .no_deps()
        .exec()
        .map_err(io::Error::other)?;
    let target = metadata.target_directory.as_std_path();
    let mut paths = Vec::new();
    for line in io::stdin().lock().lines() {
        let event: Event = serde_json::from_str(&line?).map_err(io::Error::other)?;
        paths.extend(event.tags.into_iter().filter_map(|tag| match tag {
            Tag::Path { absolute } => Some(absolute),
            Tag::Other => None,
        }));
    }
    if !paths.is_empty() && paths.iter().all(|path| path.starts_with(target)) {
        return Ok(ExitCode::SUCCESS);
    }
    let args = args.strip_prefix(&[OsString::from("--")]).unwrap_or(args);
    Ok(super::exit_code(Command::new(cargo).args(args).status()?))
}

#[derive(Deserialize)]
struct Event {
    tags: Vec<Tag>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Tag {
    Path {
        absolute: PathBuf,
    },
    #[serde(other)]
    Other,
}
