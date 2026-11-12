//! Parse retained browser inputs before starting any browser work.

use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashSet, io};

/// Parsed export selections in their authored execution order.
pub(super) struct Inventory {
    /// Named families with validated unique identifiers.
    pub families: [(&'static str, Vec<String>); 4],
    /// The one admitted policy exception.
    pub expected_error: ExpectedError,
}

/// Authored inventory fields before cross-family checks.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawInventory {
    /// Execution assertions.
    execution: Vec<String>,
    /// Loading assertions.
    loading: Vec<String>,
    /// Contribution assertions.
    contributions: Vec<String>,
    /// Assertions that mount on the canvas page's `#canvas`.
    canvas: Vec<String>,
    /// Designated policy exception.
    expected_error: ExpectedError,
}

/// Exact exception identity and required per-export count.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct ExpectedError {
    /// Export allowed to emit this exception.
    pub export: String,
    /// JavaScript exception class.
    pub name: String,
    /// First-line exception message without the class prefix.
    pub message: String,
    /// Exception document URL.
    pub source: String,
    /// Required number of matching exceptions.
    pub count: usize,
}

/// Read the four export families and their designated policy error.
pub(super) fn inventory(text: &str) -> io::Result<Inventory> {
    let raw: RawInventory = serde_json::from_str(text).map_err(io::Error::other)?;
    let value = Inventory {
        families: [
            ("execution", raw.execution),
            ("loading", raw.loading),
            ("contributions", raw.contributions),
            ("canvas", raw.canvas),
        ],
        expected_error: raw.expected_error,
    };
    let mut names = HashSet::new();
    for ((family, selected), count) in value.families.iter().zip([27, 70, 189, 165]) {
        if selected.len() != count {
            return Err(io::Error::other(format!("{family} inventory count")));
        }
        for name in selected {
            if !valid_name(name) {
                return Err(io::Error::other(format!("invalid {family} export")));
            }
            if !names.insert(name) {
                return Err(io::Error::other(format!("duplicate export {name}")));
            }
        }
    }
    if value.expected_error
        != (ExpectedError {
            export: "inline_programs_preserve_text_policies_and_order".into(),
            name: "ReferenceError".into(),
            message: "__maestro0 is not defined".into(),
            source: "about:srcdoc".into(),
            count: 1,
        })
    {
        return Err(io::Error::other("invalid expected policy error"));
    }
    Ok(value)
}

/// Accept only authored lowercase snake-case export identifiers.
fn valid_name(name: &str) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// Require the exact version supplied by the pinned browser job.
pub(super) fn chromium_version(text: &str) -> io::Result<()> {
    if text
        .split(|character: char| !character.is_ascii_digit() && character != '.')
        .any(|word| word == "156.0.8078.4")
    {
        Ok(())
    } else {
        Err(io::Error::other("chromium version must be 156.0.8078.4"))
    }
}

/// Select the one emitted WebAssembly compiler artifact.
pub(super) fn artifact(text: &str, kind: &str, name: &str) -> io::Result<String> {
    let mut files = Vec::new();
    for line in text.lines().filter(|line| line.starts_with('{')) {
        let message: Value = serde_json::from_str(line).map_err(io::Error::other)?;
        if message["reason"] != "compiler-artifact"
            || message["target"]["name"] != name
            || !message["target"]["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|value| value == kind))
        {
            continue;
        }
        let filenames = message["filenames"]
            .as_array()
            .ok_or_else(|| io::Error::other("artifact filenames"))?;
        for file in filenames {
            let file = file
                .as_str()
                .ok_or_else(|| io::Error::other("artifact filename"))?;
            let name = maestro_path::basename(file, None);
            if name
                .rsplit_once('.')
                .is_some_and(|(_, extension)| extension == "wasm")
            {
                files.push(file.to_owned());
            }
        }
    }
    match files.as_slice() {
        [file] => Ok(file.clone()),
        _ => Err(io::Error::other(format!(
            "expected one emitted {kind} wasm artifact for {name}"
        ))),
    }
}
