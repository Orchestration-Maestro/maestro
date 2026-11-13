//! Shared contract rows that every crate step edits stay in order.

use std::path::{Path, PathBuf};

use crate::graph::policy::POLICY;

/// The crate table that documents the enforced graph, named by its header row.
const CRATE_TABLE_HEADER: &str = "| Crate | One job | Allowed internal dependencies | Layer |";

/// The CI arrays that every crate step extends.
const CI_ARRAYS: [&str; 2] = ["wasm_crates", "test_prepare_inputs"];

/// The CI keys that route one repository path to the packages that read it.
const ROUTE_PREFIX: &str = "test_inputs.\"";

/// The policy table's own source, named in its diagnostics.
const POLICY_SOURCE: &str = "crates/maestro-test-conventions/src/graph/policy/table.rs";

/// The specification that documents the enforced graph and its crate table.
const SPECIFICATION: &str = "docs/specs/maestro-port.md";

/// The CI configuration that selects and prepares the workspace tests.
const CONFIGURATION: &str = ".github/ci.toml";

/// Reject a shared contract row that does not sit where its name sorts.
///
/// Every crate step adds its own row to these files, so a row written where its name sorts
/// keeps two steps on separate lines and a row appended at the end puts every step on the
/// same one. A CI array written on its key's line is reported the same way, because no
/// writer can insert into it on one line without rewriting the line another step owns.
/// A tree that holds neither file is not this check's subject.
///
/// # Errors
///
/// Returns one diagnostic per table or array whose rows are out of order or whose entries
/// share a line.
pub(crate) fn check(root: &Path) -> Result<(), String> {
    let mut findings = policy_findings();
    if let Some((path, text)) = contract_file(root, SPECIFICATION)? {
        findings.extend(crate_table_findings(&path, &text));
    }
    if let Some((path, text)) = contract_file(root, CONFIGURATION)? {
        findings.extend(array_findings(&path, &text));
    }
    if findings.is_empty() {
        return Ok(());
    }
    Err(findings.join("\n"))
}

/// Read a contract file below `root`, or `None` when the tree holds none.
fn contract_file(root: &Path, path: &str) -> Result<Option<(PathBuf, String)>, String> {
    let full = root.join(path);
    match std::fs::read_to_string(&full) {
        Ok(text) => Ok(Some((full, text))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{}: {error}", full.display())),
    }
}

/// The first policy row that does not sort after the row above it.
fn policy_findings() -> Vec<String> {
    let names: Vec<&str> = POLICY.iter().map(|row| row.0).collect();
    let Some(position) = first_out_of_order(&names) else {
        return Vec::new();
    };
    vec![format!(
        "{POLICY_SOURCE}: policy rows are not in crate-name order: `{}` is listed before `{}`, which sorts before it",
        names[position - 1],
        names[position]
    )]
}

/// The first crate-table row that does not sort after the row above it.
fn crate_table_findings(path: &Path, text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let Some(header) = lines.iter().position(|line| *line == CRATE_TABLE_HEADER) else {
        return Vec::new();
    };
    let rows: Vec<(usize, &str)> = lines
        .iter()
        .enumerate()
        .skip(header + 2)
        .take_while(|(_, line)| line.starts_with('|'))
        .map(|(index, line)| (index + 1, row_name(line)))
        .collect();
    let names: Vec<&str> = rows.iter().map(|(_, name)| *name).collect();
    let Some(position) = first_out_of_order(&names) else {
        return Vec::new();
    };
    vec![format!(
        "{}:{}: crate table rows are not in crate-name order: `{}` is listed before `{}`, which sorts before it",
        path.display(),
        rows[position].0,
        names[position - 1],
        names[position]
    )]
}

/// The crate name of one crate-table row.
fn row_name(line: &str) -> &str {
    line.split('|')
        .nth(1)
        .unwrap_or_default()
        .trim()
        .trim_matches('`')
}

/// One finding per checked CI array that is out of order or not written one entry per line.
fn array_findings(path: &Path, text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut findings = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some((key, _)) = line.split_once(" = ") else {
            continue;
        };
        if !checked_key(key) {
            continue;
        }
        let entries = match array_entries(path, &lines, index, key) {
            Ok(entries) => entries,
            Err(layout) => {
                findings.push(layout);
                continue;
            }
        };
        let values: Vec<&str> = entries.iter().map(|(_, value)| value.as_str()).collect();
        let Some(position) = first_out_of_order(&values) else {
            continue;
        };
        findings.push(format!(
            "{}:{}: {key} entries are not in name order: `{}` is listed before `{}`, which sorts before it",
            path.display(),
            entries[position].0,
            values[position - 1],
            values[position]
        ));
    }
    findings
}

/// Whether a CI key holds an array this check keeps in order.
fn checked_key(key: &str) -> bool {
    CI_ARRAYS.contains(&key) || key.starts_with(ROUTE_PREFIX)
}

/// The entries of the array whose key line is `key_line`, each with its line number.
fn array_entries(
    path: &Path,
    lines: &[&str],
    key_line: usize,
    key: &str,
) -> Result<Vec<(usize, String)>, String> {
    let value = lines
        .get(key_line)
        .and_then(|line| line.split_once(" = "))
        .map(|(_, value)| value)
        .unwrap_or_default();
    if value == "[]" {
        return Ok(Vec::new());
    }
    if value != "[" {
        return Err(format!(
            "{}:{}: {key} opens its entries on the key line; write one entry per line",
            path.display(),
            key_line + 1
        ));
    }
    let mut entries = Vec::new();
    for (index, line) in lines.iter().enumerate().skip(key_line + 1) {
        let trimmed = line.trim();
        if trimmed == "]" {
            return Ok(entries);
        }
        let entry = trimmed.strip_suffix(',').unwrap_or(trimmed);
        if entry.matches('"').count() != 2 || !entry.starts_with('"') || !entry.ends_with('"') {
            return Err(format!(
                "{}:{}: {key} does not hold exactly one entry here; write one entry per line",
                path.display(),
                index + 1
            ));
        }
        entries.push((index + 1, entry[1..entry.len() - 1].to_owned()));
    }
    Err(format!(
        "{}:{}: {key} has no closing bracket",
        path.display(),
        key_line + 1
    ))
}

/// The position of the first name that sorts before the name listed above it.
fn first_out_of_order(names: &[&str]) -> Option<usize> {
    names
        .windows(2)
        .position(|pair| pair[0] > pair[1])
        .map(|position| position + 1)
}

#[cfg(test)]
mod tests;
