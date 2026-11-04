//! The rows a footer view renders, shared by the author harness and its component fixture.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{ExtensionResult, FooterData};
use serde_json::json;

/// The rows a footer renders: its label and the footer metadata it reads.
pub(super) fn render(label: &str, footer: &FooterData) -> ExtensionResult<Vec<String>> {
    let branch = footer.get_git_branch()?;
    let statuses = footer.get_extension_statuses()?;
    let providers = footer.get_available_provider_count()?;
    Ok(vec![
        json!([label, branch, statuses, providers]).to_string(),
    ])
}
