//! Render the extension index and the component crate's example targets from source files.

use std::ffi::OsString;
use std::fmt::Write;
use std::fs;
use std::io;
use std::path::Path;
use std::process::ExitCode;

use super::example_files;

/// Markers around the README table.
const README: [&str; 2] = ["<!-- example-index:start -->", "<!-- example-index:end -->"];
/// Markers around the manifest's example targets.
const MANIFEST: [&str; 2] = ["# example-index:start", "# example-index:end"];
/// The crate that builds every example as a component.
const CRATE: &str = "crates/maestro-extensions-wasm";
/// The crate's native preparation program, the one crate example that is not a component.
const PREPARATION: &str = "prepare_author_component";

/// One example target of the component crate.
struct Target {
    /// Cargo target name.
    name: String,
    /// Manifest path of a shipped example; crate examples use Cargo's default location.
    path: Option<String>,
}

/// Refresh the generated regions, or check them without changing files.
pub fn run(args: &[OsString], checkout: &Path) -> io::Result<ExitCode> {
    if !args.is_empty() && args != [OsString::from("--check")] {
        return Err(io::Error::other("usage: example-index [--check]"));
    }
    let folder = checkout.join("examples/extensions");
    let local = checkout.join(CRATE).join("examples");
    let targets = targets(&folder, &local)?;
    let regions = [
        (folder.join("README.md"), README, table(&folder)?),
        (
            checkout.join(CRATE).join("Cargo.toml"),
            MANIFEST,
            manifest(&targets),
        ),
    ];
    let mut changed = Vec::new();
    for (path, markers, body) in regions {
        let before = fs::read_to_string(&path)?;
        let after = splice(&path, &before, markers, &body)?;
        if before != after {
            changed.push((path, after));
        }
    }
    if !args.is_empty()
        && let Some((path, _)) = changed.first()
    {
        return Err(io::Error::other(format!(
            "{} is stale; run just example-index",
            path.display()
        )));
    }
    // Unchanged files keep their times, so the test setup's freshness check still holds.
    for (path, after) in changed {
        fs::write(path, after)?;
    }
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

/// Find the shipped examples and the crate's own component examples, in name order.
///
/// A shipped example is a top-level source file or a directory with its own `main.rs`.
fn targets(folder: &Path, local: &Path) -> io::Result<Vec<Target>> {
    let mut targets = Vec::new();
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
                name: example_files::target_name(&folder.join(&source))?,
                path: Some(format!("../../examples/extensions/{source}")),
            });
        }
    }
    for entry in fs::read_dir(local)? {
        let entry = entry?;
        let file = file_name(&entry)?;
        match file.strip_suffix(".rs") {
            Some(stem) if stem != PREPARATION && entry.file_type()?.is_file() => {
                targets.push(Target {
                    name: example_files::target_name(&entry.path())?,
                    path: None,
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
            let path = target
                .path
                .as_ref()
                .map(|path| format!("path = \"{path}\"\n"))
                .unwrap_or_default();
            format!(
                "[[example]]\nname = \"{}\"\n{path}crate-type = [\"cdylib\"]\n",
                target.name
            )
        })
        .collect();
    tables.join("\n")
}
