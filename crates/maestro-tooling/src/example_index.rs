//! Render the extension index, the component crate's example targets and its prepared
//! components from the example sources.

use maestro_resources::frontmatter::{FrontmatterValue, parse_frontmatter};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fmt::Write;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Markers around the README table.
const README: [&str; 2] = ["<!-- example-index:start -->", "<!-- example-index:end -->"];
/// Markers around the manifest's example targets.
const MANIFEST: [&str; 2] = ["# example-index:start", "# example-index:end"];
/// Markers around the preparation program's component table.
const COMPONENTS: [&str; 2] = ["// example-index:start", "// example-index:end"];
/// Opening words of a component marker, which a malformed marker shares.
const MARKER_OPENING: &str = "// Tests receive";
/// The line through which an example asks the tests for its built component, before the variable.
const MARKER: &str = "// Tests receive this component's path in ";
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
    /// Root source file, which may carry the component marker.
    source: PathBuf,
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
        (
            local.join(format!("{PREPARATION}.rs")),
            COMPONENTS,
            components(&targets)?,
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
    let mut files = Vec::new();
    collect(folder, &mut files)?;
    files.sort_by_cached_key(|path| path.to_string_lossy().replace('\\', "/"));
    let mut result = String::from("| File | Provides |\n| --- | --- |\n");
    for file in files {
        let relative = file.strip_prefix(folder).map_err(io::Error::other)?;
        let name = relative.to_string_lossy().replace('\\', "/");
        let summary = summary(&file)?.replace('|', "\\|");
        writeln!(result, "| `{name}` | {summary} |").map_err(io::Error::other)?;
    }
    Ok(result)
}

/// Gather documented files recursively, including helpers and usage pages.
fn collect(folder: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect(&entry.path(), files)?;
        } else if kind.is_file() && entry.file_name() != "README.md" {
            files.push(entry.path());
        }
    }
    Ok(())
}

/// Read source documentation, Markdown descriptions or a theme name.
fn summary(path: &Path) -> io::Result<String> {
    let text = fs::read_to_string(path)?;
    let result = if path.extension().is_some_and(|extension| extension == "rs") {
        text.lines()
            .map_while(|line| line.strip_prefix("//!"))
            .map(str::trim)
            .take_while(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    } else if path
        .extension()
        .is_some_and(|extension| extension == "json")
    {
        serde_json::from_str::<serde_json::Value>(&text)
            .map_err(io::Error::other)?
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    } else if let Some(heading) = text.lines().next().and_then(|line| line.strip_prefix("# ")) {
        heading.to_owned()
    } else {
        match parse_frontmatter(&text)
            .map_err(io::Error::other)?
            .frontmatter
        {
            FrontmatterValue::Mapping(fields) => fields
                .into_iter()
                .find_map(|(key, value)| {
                    if key == "description"
                        && let FrontmatterValue::String(description) = value
                    {
                        Some(description)
                    } else {
                        None
                    }
                })
                .unwrap_or_default(),
            _ => String::new(),
        }
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
        let (name, source) = if entry.file_type()?.is_dir() {
            (file.replace('-', "_"), format!("{file}/main.rs"))
        } else if let Some(stem) = file.strip_suffix(".rs") {
            (stem.to_owned(), file.clone())
        } else {
            continue;
        };
        if folder.join(&source).is_file() {
            targets.push(Target {
                name,
                path: Some(format!("../../examples/extensions/{source}")),
                source: folder.join(&source),
            });
        }
    }
    for entry in fs::read_dir(local)? {
        let entry = entry?;
        let file = file_name(&entry)?;
        match file.strip_suffix(".rs") {
            Some(stem) if stem != PREPARATION && entry.file_type()?.is_file() => {
                targets.push(Target {
                    name: stem.to_owned(),
                    path: None,
                    source: entry.path(),
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

/// Render the table of marked examples and the variables that carry their component paths.
fn components(targets: &[Target]) -> io::Result<String> {
    let mut result = String::from(
        "/// Each prepared example and the variable that hands its component path to the tests.\nconst COMPONENTS: &[(&str, &str)] = &[\n",
    );
    let mut variables = BTreeSet::new();
    for target in targets {
        let Some(variable) = variable(&target.source)? else {
            continue;
        };
        if !variables.insert(variable.clone()) {
            return Err(io::Error::other(format!(
                "two component markers name {variable}"
            )));
        }
        writeln!(result, "    (\"{}\", \"{variable}\"),", target.name).map_err(io::Error::other)?;
    }
    result.push_str("];\n");
    Ok(result)
}

/// Read the one component marker of a source, if it has one.
fn variable(source: &Path) -> io::Result<Option<String>> {
    let text = fs::read_to_string(source)?;
    let mut found = None;
    for line in text.lines().filter(|line| line.starts_with(MARKER_OPENING)) {
        let variable = line
            .strip_prefix(MARKER)
            .and_then(|rest| rest.strip_suffix('.'))
            .filter(|variable| well_formed(variable))
            .ok_or_else(|| {
                io::Error::other(format!(
                    "{}: malformed component marker; write `{MARKER}MAESTRO_<NAME>_COMPONENT.`",
                    source.display()
                ))
            })?;
        if found.replace(variable.to_owned()).is_some() {
            return Err(io::Error::other(format!(
                "{} has two component markers",
                source.display()
            )));
        }
    }
    Ok(found)
}

/// Whether a variable is `MAESTRO_<NAME>_COMPONENT` with a nonempty uppercase name.
fn well_formed(variable: &str) -> bool {
    variable
        .strip_prefix("MAESTRO_")
        .and_then(|rest| rest.strip_suffix("_COMPONENT"))
        .is_some_and(|name| {
            !name.is_empty()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        })
}
