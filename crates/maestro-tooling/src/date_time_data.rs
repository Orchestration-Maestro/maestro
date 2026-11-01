//! Regenerate fixed-profile date-time assets from verified offline JSON.

use serde::{Deserialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs, io,
    path::Path,
    process::ExitCode,
};

/// Relative directory containing the generated tables and their recorded inputs.
const DATA: &str = "crates/maestro-text/data/date-time";
/// SHA-256 of the manifest pinning source archives, runtime versions and input bytes.
const MANIFEST_HASH: &str = "d083225c1602b0690ef0e9fced904bb78c4d7f2c6fee002ef4814d860653b429";
/// Source locales, expanded locale rows and unique recipes in the pinned recording.
const DOMAIN_COUNTS: [usize; 3] = [766, 2301, 444];

/// Hashes of every input consumed by the generator.
#[derive(Deserialize)]
struct Manifest {
    /// File names relative to the inputs directory, with lowercase SHA-256 hashes.
    files: BTreeMap<String, String>,
}

/// Recorded numeric patterns and their regional locale index.
#[derive(Deserialize)]
struct Oracle {
    /// Locale identifiers paired with zero-based recipe indices.
    rows: Vec<(String, usize)>,
    /// Calendar, digits, cycle, pattern, morning, evening and era fields.
    records: Vec<[String; 7]>,
}

/// CLDR's preferred clock for one region or language-region combination.
#[derive(Deserialize)]
struct Hour {
    /// Preferred skeleton hour letter.
    #[serde(rename = "_preferred")]
    preferred: String,
}

/// Regenerate the tables, or check their exact bytes without changing files.
pub(super) fn run(args: &[OsString], checkout: &Path) -> io::Result<ExitCode> {
    if !args.is_empty() && args != [OsString::from("--check")] {
        return Err(io::Error::other("usage: date-time-data [--check]"));
    }
    let directory = checkout.join(DATA);
    let inputs = inputs(&directory.join("inputs"))?;
    let assets = assets(&inputs)?;
    for (name, content) in assets {
        let path = directory.join(name);
        if args.is_empty() {
            fs::write(path, content)?;
        } else if fs::read(&path)? != content {
            return Err(io::Error::other(format!("{} is stale", path.display())));
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Read and authenticate every input before any generated file is written.
fn inputs(directory: &Path) -> io::Result<BTreeMap<String, Vec<u8>>> {
    let manifest = verified(&directory.join("manifest.json"), MANIFEST_HASH)?;
    let manifest: Manifest = serde_json::from_slice(&manifest).map_err(io::Error::other)?;
    manifest
        .files
        .into_iter()
        .map(|(name, hash)| {
            let bytes = verified(&directory.join(&name), &hash)?;
            Ok((name, bytes))
        })
        .collect()
}

/// Refuse missing or changed bytes, including a replaced hash manifest.
fn verified(path: &Path, expected: &str) -> io::Result<Vec<u8>> {
    let bytes = fs::read(path)
        .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", path.display())))?;
    let actual = crate::test_setup::hex(&Sha256::digest(&bytes));
    if actual != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: SHA-256 mismatch", path.display()),
        ));
    }
    Ok(bytes)
}

/// Decode a verified JSON document or one named value within it.
fn json<T: DeserializeOwned>(
    inputs: &BTreeMap<String, Vec<u8>>,
    name: &str,
    pointer: &str,
) -> io::Result<T> {
    let bytes = inputs
        .get(name)
        .ok_or_else(|| io::Error::other(format!("missing input: {name}")))?;
    let document: serde_json::Value = serde_json::from_slice(bytes).map_err(io::Error::other)?;
    let value = document
        .pointer(pointer)
        .ok_or_else(|| io::Error::other(format!("missing {name}{pointer}")))?;
    serde_json::from_value(value.clone()).map_err(io::Error::other)
}

/// Produce all five assets in memory before publishing any of them.
fn assets(inputs: &BTreeMap<String, Vec<u8>>) -> io::Result<BTreeMap<&'static str, Vec<u8>>> {
    let oracle: Oracle = json(inputs, "oracle.json", "")?;
    let locales: Vec<String> = json(inputs, "source-locales.json", "")?;
    let indexed: BTreeSet<_> = oracle.rows.iter().map(|(locale, _)| locale).collect();
    if [locales.len(), oracle.rows.len(), oracle.records.len()] != DOMAIN_COUNTS
        || oracle
            .rows
            .iter()
            .any(|(_, id)| *id >= oracle.records.len())
        || locales.iter().any(|locale| !indexed.contains(locale))
    {
        return Err(io::Error::other("invalid recorded locale domain"));
    }
    let hours: BTreeMap<String, Hour> = json(inputs, "timeData.json", "/supplemental/timeData")?;
    Ok(BTreeMap::from([
        (
            "index.tsv",
            tsv(oracle
                .rows
                .into_iter()
                .map(|(locale, id)| vec![locale, id.to_string()]))?,
        ),
        (
            "recipes.tsv",
            tsv(oracle.records.into_iter().map(Vec::from))?,
        ),
        (
            "hours.tsv",
            tsv(hours.into_iter().map(|(region, hour)| {
                vec![
                    region,
                    if hour.preferred == "h" { "h12" } else { "h23" }.into(),
                ]
            }))?,
        ),
        ("calendars.tsv", calendars(inputs)?),
        (
            "LICENSE",
            inputs
                .get("LICENSE")
                .ok_or_else(|| io::Error::other("missing licence input"))?
                .clone(),
        ),
    ]))
}

/// Retain only territory defaults that differ from the Gregorian calendar.
fn calendars(inputs: &BTreeMap<String, Vec<u8>>) -> io::Result<Vec<u8>> {
    let calendars: BTreeMap<String, Vec<String>> = json(
        inputs,
        "calendarPreferenceData.json",
        "/supplemental/calendarPreferenceData",
    )?;
    let mut defaults = Vec::new();
    for (region, preferences) in calendars {
        let first = preferences
            .into_iter()
            .next()
            .ok_or_else(|| io::Error::other("empty calendar preferences"))?;
        match first.as_str() {
            "gregorian" => {}
            "persian" | "buddhist" => defaults.push(vec![region, first]),
            _ => return Err(io::Error::other("unsupported default calendar")),
        }
    }
    tsv(defaults)
}

/// Encode delimiter-free rows, preserving significant empty trailing fields.
fn tsv(rows: impl IntoIterator<Item = Vec<String>>) -> io::Result<Vec<u8>> {
    let mut output = String::new();
    for row in rows {
        if row.iter().any(|field| field.contains(['\t', '\r', '\n'])) {
            return Err(io::Error::other("delimiter in date-time data field"));
        }
        output.push_str(&row.join("\t"));
        output.push('\n');
    }
    Ok(output.into_bytes())
}
