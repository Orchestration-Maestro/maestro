//! Finite component preparation scenarios for copied author-example artifacts.

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use maestro_test_process::child::write_stdout;
use serde::Deserialize;

/// Engine identities whose namespaces the preparation tests distinguish.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Engine {
    /// The engine used for the first prepared bundle.
    Fixture,
    /// An independently compiled engine replacing the first one.
    Newer,
}

/// An author example's complete behavior, parsed before any output or file mutation.
#[derive(Debug, Deserialize)]
#[serde(tag = "scenario", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Record {
    /// Export supplied components and populate the named engine's cache.
    Cached {
        /// Engine whose compiled code this example writes.
        engine: Engine,
    },
    /// Export components without populating the cache.
    Plain,
    /// Write an environment entry the setup must refuse.
    Malformed,
    /// Change a source after exporting components and compiled code.
    Unstable,
    /// Populate a namespace whose time disagrees with this executable.
    Misplaced,
    /// Populate two independent namespaces.
    Split,
    /// Build only components missing from the supplied environment, with real dependency files.
    Marked,
}

impl Record {
    /// Decode one finite author record after its family marker.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let Some(("tooling-author", json)) = text.split_once('\n') else {
            return Err(io::Error::other("expected tooling-author and one scenario"));
        };
        serde_json::from_str(json).map_err(io::Error::other)
    }
}

/// Query the selected cache or run the recorded preparation without interpreting source text.
pub(super) fn run(record: &Record, args: &[OsString]) -> io::Result<()> {
    match args {
        [argument] if argument == "--cache-directory" => {
            write_stdout(&serde_json::to_vec(&cache_directory()?)?)?;
            return write_stdout(b"\n");
        }
        [_workspace, _component, _fixed] => {}
        _ => return Err(io::Error::other("unknown author preparation arguments")),
    }
    let environment = std::env::var_os("NEXTEST_ENV")
        .ok_or_else(|| io::Error::other("missing author environment output"))?;
    match record {
        Record::Malformed => std::fs::write(environment, "invalid\n"),
        Record::Marked => marked(Path::new(&environment)),
        _ => {
            entries(record, Path::new(&environment))?;
            warm(record)?;
            if matches!(record, Record::Unstable) {
                std::fs::write("crates/maestro-extensions-wasm/src/lib.rs", "edited\n")?;
            }
            Ok(())
        }
    }
}

/// Resolve the fixture's absolute XDG or home-based cache without creating it during a query.
fn cache_directory() -> io::Result<PathBuf> {
    let cache = maestro_test_process::user_cache_home()
        .ok_or_else(|| io::Error::other("missing fixture cache home"))?;
    if !cache.is_absolute() {
        return Err(io::Error::other("fixture cache home must be absolute"));
    }
    Ok(cache.join("wasmtime"))
}

/// Export the ordinary fixed components, retaining prebuilt values where the scenario uses them.
fn entries(record: &Record, environment: &Path) -> io::Result<()> {
    let supplied = matches!(record, Record::Cached { .. } | Record::Unstable);
    if supplied {
        append(
            "seen",
            &std::env::var("MAESTRO_AUTHOR_COMPONENT").unwrap_or_else(|_| "unset".into()),
        )?;
    }
    let fallback = std::env::current_dir()?.join("component.wasm");
    let variables = [
        "MAESTRO_AUTHOR_COMPONENT",
        "MAESTRO_CONTEXT_COMPONENT",
        "MAESTRO_SUBAGENT_COMPONENT",
    ];
    let mut file = std::fs::File::create(environment)?;
    for variable in variables
        .into_iter()
        .chain(supplied.then_some("MAESTRO_COMPONENT"))
    {
        let value = supplied.then(|| std::env::var_os(variable)).flatten();
        let path = value
            .filter(|value| !value.is_empty())
            .map_or_else(|| fallback.clone(), PathBuf::from);
        writeln!(file, "{variable}={}", path.display())?;
    }
    Ok(())
}

/// Write the recorded cache namespaces, preserving an entry supplied by a base bundle.
fn warm(record: &Record) -> io::Result<()> {
    if std::env::var_os("XDG_CACHE_HOME").is_none() {
        return Ok(());
    }
    let time = std::fs::metadata(std::env::current_exe()?)?
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_millis();
    let names = match record {
        Record::Cached { engine } => vec![format!(
            "{}-{time}",
            match engine {
                Engine::Fixture => "wasmtime-fixture",
                Engine::Newer => "wasmtime-newer",
            }
        )],
        Record::Unstable => vec![format!("wasmtime-fixture-{time}")],
        Record::Misplaced => vec!["wasmtime-fixture-1".into()],
        Record::Split => vec![
            format!("wasmtime-fixture-{time}"),
            format!("wasmtime-other-{time}"),
        ],
        Record::Plain | Record::Malformed | Record::Marked => return Ok(()),
    };
    for name in names {
        let directory = cache_directory()?.join("modules").join(name);
        std::fs::create_dir_all(&directory)?;
        if !directory.join("key").exists() {
            let value = if std::env::var_os("MAESTRO_AUTHOR_COMPONENT")
                .is_some_and(|value| !value.is_empty())
            {
                "recompiled"
            } else {
                "compiled"
            };
            std::fs::write(directory.join("key"), value)?;
            std::fs::write(directory.join("key.stats"), "stats")?;
        }
    }
    Ok(())
}

/// Build each missing marked component and describe its independent source inputs.
fn marked(environment: &Path) -> io::Result<()> {
    let root = std::env::current_dir()?;
    let library = root.join("crates/maestro-extensions-wasm/src/lib.rs");
    let mut file = std::fs::File::options()
        .append(true)
        .create(true)
        .open(environment)?;
    for variable in [
        "MAESTRO_AUTHOR_COMPONENT",
        "MAESTRO_CONTEXT_COMPONENT",
        "MAESTRO_SUBAGENT_COMPONENT",
        "MAESTRO_ALPHA_COMPONENT",
        "MAESTRO_BETA_COMPONENT",
    ] {
        let supplied = std::env::var_os(variable).filter(|value| !value.is_empty());
        let path = if let Some(value) = supplied {
            PathBuf::from(value)
        } else {
            append("built", variable)?;
            let path = root.join(format!("{variable}.wasm"));
            std::fs::copy(root.join("component.wasm"), &path)?;
            let mut sources = vec![library.clone()];
            if let Some(name) = match variable {
                "MAESTRO_ALPHA_COMPONENT" => Some("alpha"),
                "MAESTRO_BETA_COMPONENT" => Some("beta"),
                _ => None,
            } {
                sources.push(root.join(format!("examples/extensions/{name}.rs")));
                sources.push(root.join("examples/extensions/support/tail.rs"));
            }
            dependencies(&path, &sources)?;
            path
        };
        writeln!(file, "{variable}={}", path.display())?;
    }
    Ok(())
}

/// Write Cargo-shaped target and source rules beside one component.
fn dependencies(component: &Path, sources: &[PathBuf]) -> io::Result<()> {
    let mut file = std::fs::File::create(component.with_extension("d"))?;
    write!(file, "{}:", component.display())?;
    for source in sources {
        write!(file, " {}", source.display())?;
    }
    writeln!(file, "\n")?;
    for source in sources {
        writeln!(file, "{}:", source.display())?;
    }
    Ok(())
}

/// Append one complete observation to the checkout-local log.
fn append(path: &str, value: &str) -> io::Result<()> {
    writeln!(
        std::fs::File::options()
            .append(true)
            .create(true)
            .open(path)?,
        "{value}"
    )
}
