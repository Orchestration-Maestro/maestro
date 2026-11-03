//! Parse retained browser inputs before starting any browser work.

use serde_json::{Value, json};
use std::{collections::HashSet, io};

/// Read the three export families and their designated policy error.
pub(super) fn inventory(text: &str) -> io::Result<Value> {
    let value: Value = serde_json::from_str(text).map_err(io::Error::other)?;
    let object = value
        .as_object()
        .ok_or_else(|| io::Error::other("missing inventory"))?;
    if object.len() != 4 {
        return Err(io::Error::other("inventory fields"));
    }
    let mut names = HashSet::new();
    for (family, count) in [("execution", 26), ("loading", 49), ("contributions", 194)] {
        let selected = object
            .get(family)
            .and_then(Value::as_array)
            .filter(|selected| selected.len() == count)
            .ok_or_else(|| io::Error::other(format!("{family} inventory count")))?;
        for name in selected {
            let name = name
                .as_str()
                .filter(|name| valid_name(name))
                .ok_or_else(|| io::Error::other(format!("invalid {family} export")))?;
            if !names.insert(name) {
                return Err(io::Error::other(format!("duplicate export {name}")));
            }
        }
    }
    if value["expectedError"]
        != json!({"export":"inline_programs_preserve_text_policies_and_order", "name":"TypeError", "message":"window.complete is not a function", "source":"about:srcdoc", "count":1})
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
