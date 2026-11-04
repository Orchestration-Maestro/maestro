//! Render the extension index, component example targets and standalone guide lock.

use std::ffi::OsString;
use std::fmt::Write;
use std::fs;
use std::io;
use std::path::Path;
use std::process::ExitCode;

use super::example_files;
use super::workspace::{self, FIXTURES, Workspace};

/// Offline resolution of the standalone guide's generated lock.
mod guide_lock;

/// Markers around the README table.
const README: [&str; 2] = ["<!-- example-index:start -->", "<!-- example-index:end -->"];
/// Markers around the manifest's example targets.
const MANIFEST: [&str; 2] = [workspace::REGION_START, workspace::REGION_END];
/// The crate whose examples the guide project builds on.
const GUEST: &str = "maestro-extensions-wasm";
/// The crate's native preparation program, the one crate example that is not a component.
const PREPARATION: &str = "prepare_author_component";
/// One example target of the component crate.
struct Target {
    /// Cargo target name.
    name: String,
    /// Manifest path of the target's source, relative to the crate.
    path: String,
}

/// Refresh the generated regions and guide lock, or check them without changing files.
///
/// # Errors
/// Returns an unknown argument, a workspace without one component crate, a missing marker,
/// stale output in check mode, or a failed read, render or write.
pub fn run(args: &[OsString], cargo: &Path, checkout: &Path) -> io::Result<ExitCode> {
    if !args.is_empty() && args != [OsString::from("--check")] {
        return Err(io::Error::other("usage: example-index [--check]"));
    }
    let check = !args.is_empty();
    let workspace = Workspace::read(cargo, checkout)?;
    let component = workspace.component_crate()?;
    let shipped = workspace.shipped();
    let targets = targets(&workspace, &component, shipped.as_deref())?;
    let mut regions = Vec::new();
    if let Some(folder) = &shipped {
        regions.push((folder.join("README.md"), README, table(folder)?));
    }
    regions.push((component.join("Cargo.toml"), MANIFEST, manifest(&targets)));
    let mut changed = Vec::new();
    for (path, markers, body) in regions {
        let before = fs::read_to_string(&path)?;
        let after = splice(&path, &before, markers, &body)?;
        if before != after {
            changed.push((path, after));
        }
    }
    if check && let Some((path, _)) = changed.first() {
        return Err(io::Error::other(format!(
            "{} is stale; run just example-index",
            path.display()
        )));
    }
    // Unchanged files keep their times, so the test setup's freshness check still holds.
    for (path, after) in changed {
        fs::write(path, after)?;
    }
    let guest = workspace.package(GUEST)?;
    guide_lock::refresh(
        cargo,
        checkout,
        workspace.guide().as_deref(),
        guest.as_deref(),
        check,
    )?;
    Ok(ExitCode::SUCCESS)
}

/// Replace only the marked region, retaining all surrounding text.
fn splice(path: &Path, before: &str, [start, end]: [&str; 2], body: &str) -> io::Result<String> {
    let missing = |marker| io::Error::other(format!("{} lacks {marker}", path.display()));
    let (prefix, rest) = before.split_once(start).ok_or_else(|| missing(start))?;
    let (_, suffix) = rest.split_once(end).ok_or_else(|| missing(end))?;
    Ok(format!("{prefix}{start}\n{body}{end}{suffix}"))
}

/// Render the README table rows from every documented file.
fn table(folder: &Path) -> io::Result<String> {
    let files = example_files::collect(folder)?;
    let mut result = String::from("| File | Provides |\n| --- | --- |\n");
    for file in files {
        let relative = file.strip_prefix(folder).map_err(io::Error::other)?;
        let name = relative.to_string_lossy().replace('\\', "/");
        let summary = summary(&file)?.replace('|', "\\|");
        writeln!(result, "| `{name}` | {summary} |").map_err(io::Error::other)?;
    }
    Ok(result)
}

/// Read the initial Rust documentation paragraph or Markdown page heading.
fn summary(path: &Path) -> io::Result<String> {
    let text = fs::read_to_string(path)?;
    let result = if path.extension().is_some_and(|extension| extension == "rs") {
        text.lines()
            .map_while(|line| line.strip_prefix("//!"))
            .map(str::trim)
            .take_while(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        text.lines()
            .next()
            .and_then(|line| line.strip_prefix("# "))
            .unwrap_or_default()
            .to_owned()
    };
    if result.is_empty() {
        return Err(io::Error::other(format!(
            "{} needs opening documentation for the example index",
            path.display()
        )));
    }
    Ok(result)
}

/// Find the shipped examples and the crate's own fixture components, in name order.
///
/// A shipped example is a top-level source file or a directory with its own `main.rs`. A
/// workspace without the shipped folder renders the crate's own examples alone.
fn targets(
    workspace: &Workspace,
    component: &Path,
    shipped: Option<&Path>,
) -> io::Result<Vec<Target>> {
    let mut targets = Vec::new();
    if let Some(folder) = shipped {
        let climb = workspace.climb(component)?;
        for entry in fs::read_dir(folder)? {
            let entry = entry?;
            let file = file_name(&entry)?;
            let source = if entry.file_type()?.is_dir() {
                format!("{file}/main.rs")
            } else if file.strip_suffix(".rs").is_some() {
                file.clone()
            } else {
                continue;
            };
            if folder.join(&source).is_file() {
                targets.push(Target {
                    name: example_files::target_name(folder, &folder.join(&source))?,
                    path: format!("{climb}examples/extensions/{source}"),
                });
            }
        }
    }
    let local = component.join(FIXTURES);
    for entry in fs::read_dir(&local)? {
        let entry = entry?;
        let file = file_name(&entry)?;
        match file.strip_suffix(".rs") {
            Some(stem) if stem != PREPARATION && entry.file_type()?.is_file() => {
                targets.push(Target {
                    name: example_files::target_name(&local, &entry.path())?,
                    path: format!("{FIXTURES}/{file}"),
                });
            }
            _ => {}
        }
    }
    targets.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(targets)
}

/// Decode an entry's file name, which every target name derives from.
fn file_name(entry: &fs::DirEntry) -> io::Result<String> {
    entry.file_name().into_string().map_err(|name| {
        io::Error::other(format!("example file name {} is not UTF-8", name.display()))
    })
}

/// Render one `[[example]]` table per target.
fn manifest(targets: &[Target]) -> String {
    let tables: Vec<String> = targets
        .iter()
        .map(|target| {
            format!(
                "[[example]]\nname = \"{}\"\npath = \"{}\"\ncrate-type = [\"cdylib\"]\n",
                target.name, target.path
            )
        })
        .collect();
    tables.join("\n")
}
