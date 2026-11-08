//! Compile the browser compile witnesses and capture their diagnostics.

use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, ExitCode};

/// Each witness's owning package, example target and source path.
///
/// A witness's body only compiles for the browser target, so native Clippy never
/// reads it. Every witness the workspace keeps is listed here.
pub(crate) const WITNESSES: [(&str, &str, &str); 4] = [
    (
        "maestro-catalog",
        "browser_catalog_check",
        "crates/maestro-catalog/tests/support/browser_catalog_check.rs",
    ),
    (
        "maestro-models",
        "bedrock_browser_witness",
        "crates/maestro-models/tests/support/bedrock_browser.rs",
    ),
    (
        "maestro-models",
        "browser_import_check",
        "crates/maestro-models/tests/support/browser_import_check.rs",
    ),
    (
        "maestro-models",
        "cloud_content_browser_witness",
        "crates/maestro-models/tests/support/cloud_content_browser.rs",
    ),
];

/// Compile every witness for the browser target, stopping at the first failure.
///
/// # Errors
/// Returns native process and filesystem failures, and a witness whose source is absent.
pub(crate) fn witnesses(cargo: &Path, checkout: &Path) -> io::Result<ExitCode> {
    for (package, example, source) in WITNESSES {
        if !checkout.join(source).is_file() {
            return Err(io::Error::other(format!(
                "{example} entry is not delivered"
            )));
        }
        let mut child = Command::new(cargo);
        child.current_dir(checkout).args([
            "check",
            "--locked",
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            package,
            "--example",
            example,
        ]);
        let code = run(
            &mut child,
            &std::env::temp_dir().join("maestro-browser-smoke-errors.log"),
        )?;
        if code != ExitCode::SUCCESS {
            return Ok(code);
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Compile with a supplied command, logging diagnostics on failure.
///
/// # Errors
/// Returns native process and filesystem failures.
fn run(child: &mut Command, log: &Path) -> io::Result<ExitCode> {
    let diagnostics = match child.output() {
        Ok(output) if output.status.success() => return Ok(ExitCode::SUCCESS),
        Ok(output) => output.stderr,
        Err(error) => error.to_string().into_bytes(),
    };
    fs::write(log, diagnostics)?;
    eprintln!("Browser smoke check failed. See {}", log.display());
    Ok(ExitCode::FAILURE)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use super::WITNESSES;

    /// The examples the shared wasm job compiles, grouped by owning package.
    fn declared() -> BTreeMap<String, Vec<String>> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/ci.toml");
        let declaration: toml::Table = std::fs::read_to_string(path)
            .expect("the repository declares its CI settings")
            .parse()
            .expect("the CI settings read as TOML");
        let examples = declaration
            .get("wasm_examples")
            .and_then(toml::Value::as_table)
            .expect("the CI settings declare the wasm examples");
        examples
            .iter()
            .map(|(package, names)| {
                let names = names
                    .as_array()
                    .expect("a wasm example list")
                    .iter()
                    .map(|name| name.as_str().expect("an example name").to_owned())
                    .collect();
                (package.clone(), names)
            })
            .collect()
    }

    /// The shared wasm job compiles the examples the caller declares, so the declaration names
    /// the witness list and nothing else.
    #[test]
    fn the_declared_wasm_examples_are_the_browser_witnesses() {
        let mut expected: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (package, example, _) in WITNESSES {
            expected
                .entry(package.to_owned())
                .or_default()
                .push(example.to_owned());
        }
        assert_eq!(
            declared(),
            expected,
            "the shared wasm job compiles exactly the browser witnesses"
        );
    }
}
