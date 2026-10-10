//! Native Git qualification of local package cache ignores.
#![cfg(not(target_arch = "wasm32"))]

mod support;

use std::fs;
use std::path::Path;
use std::process::Command;
use support::Workspace;

/// Run Git without inherited repository or ignore configuration.
fn git(root: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(root);
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("GIT_") {
            command.env_remove(name);
        }
    }
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("LC_ALL", "C")
        .args(["-c", "core.excludesFile=/dev/null"]);
    command
}

#[test]
fn maestro_local_caches_keep_ignore_files_trackable() {
    let workspace = Workspace::new();
    let shipped = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    assert!(
        git(&workspace.root)
            .args(["init", "-q"])
            .status()
            .unwrap()
            .success()
    );
    for path in [
        ".gitignore",
        ".maestro/git/.gitignore",
        ".maestro/npm/.gitignore",
    ] {
        let destination = workspace.root.join(path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(shipped.join(path), destination).unwrap();
    }
    let mut cases = Vec::new();
    for path in [
        ".gitignore",
        ".maestro/prompts/cl.md",
        ".maestro/prompts/is.md",
        ".maestro/prompts/pr.md",
        ".maestro/prompts/wr.md",
        ".maestro/prompts/review.md",
        "crates/probe/src/lib.rs",
    ] {
        cases.push((path.to_owned(), false));
    }
    for cache in ["git", "npm"] {
        for path in [
            "cache.txt",
            "owner/repo/file",
            ".hidden",
            "path with spaces/data",
            "nested/.gitignore",
        ] {
            cases.push((format!(".maestro/{cache}/{path}"), true));
        }
        cases.push((format!(".maestro/{cache}/.gitignore"), false));
    }
    for (path, ignored) in cases {
        check_ignore(&workspace.root, &path, ignored).unwrap();
    }
}

/// Create an untracked path without overwriting shipped ignore files and check Git's decision.
fn check_ignore(root: &Path, path: &str, ignored: bool) -> std::io::Result<()> {
    let destination = root.join(path);
    fs::create_dir_all(
        root.join(path)
            .parent()
            .ok_or_else(|| std::io::Error::other("case has no parent"))?,
    )?;
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
    {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let output = git(root)
        .args(["check-ignore", "--no-index", "-q", "--", path])
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(i32::from(!ignored)),
        "ignore decision for {path}"
    );
    assert!(output.stderr.is_empty(), "Git diagnostic for {path}");
    Ok(())
}
