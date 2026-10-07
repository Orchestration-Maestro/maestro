use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use crate::{array, source, string};

mod wit;
use wit::{ScanContext, WitInputs};

pub(crate) fn check(metadata: &Value) -> Result<(), String> {
    let members = array(metadata, "workspace_members")?;
    let mut declarations = BTreeSet::new();
    let guest = array(metadata, "packages")?
        .iter()
        .find(|package| {
            members.contains(&package["id"]) && package["name"] == "maestro-extensions-wasm"
        })
        .map(|package| {
            Path::new(string(package, "manifest_path")?)
                .parent()
                .ok_or("manifest has no parent")?
                .canonicalize()
                .map_err(|error| format!("cannot resolve guest member: {error}"))
        })
        .transpose()?;
    let mut wit = WitInputs::default();
    for package in array(metadata, "packages")? {
        if !members.contains(&package["id"]) {
            continue;
        }
        let owner = string(package, "name")?;
        let manifest = Path::new(string(package, "manifest_path")?);
        for path in source::files(manifest.parent().ok_or("manifest has no parent")?)? {
            let contents = std::fs::read_to_string(&path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            let tokens = source::tokens(&contents);
            wit.scan(
                &ScanContext {
                    owner,
                    directory: manifest.parent().ok_or("manifest has no parent")?,
                    guest: guest.as_deref(),
                    path: &path,
                    contents: &contents,
                },
                &tokens,
            )?;
            check_declarations(owner, &path, &contents, &tokens, &mut declarations)?;
        }
    }
    wit.finish()
}

fn selector(name: &str) -> bool {
    matches!(
        name,
        "ConfigSelectorComponent"
            | "ExtensionSelectorComponent"
            | "ModelSelectorComponent"
            | "OAuthSelectorComponent"
            | "ScopedModelsSelectorComponent"
            | "SessionSelectorComponent"
            | "SettingsSelectorComponent"
            | "ShowImagesSelectorComponent"
            | "ThemeSelectorComponent"
            | "ThinkingSelectorComponent"
            | "TreeSelectorComponent"
            | "UserMessageSelectorComponent"
            | "config_selector"
            | "extension_selector"
            | "model_selector"
            | "oauth_selector"
            | "scoped_models_selector"
            | "session_selector"
            | "settings_selector"
            | "show_images_selector"
            | "theme_selector"
            | "thinking_selector"
            | "tree_selector"
            | "user_message_selector"
            | "session_selector_search"
    )
}

fn check_declarations(
    owner: &str,
    path: &Path,
    contents: &str,
    tokens: &[source::Token<'_>],
    declarations: &mut BTreeSet<String>,
) -> Result<(), String> {
    for pair in tokens.windows(2) {
        if !matches!(
            pair[0].text,
            "struct" | "enum" | "union" | "trait" | "type" | "mod"
        ) {
            continue;
        }
        let name = pair[1].text;
        if selector(name) && owner != "maestro-chat" {
            let line = source::line(contents, pair[0].start);
            return Err(format!(
                "{}:{line}: {name} declaration belongs to maestro-chat, not {owner}",
                path.display()
            ));
        }
        if matches!(
            name,
            "ToolDefinition" | "ToolRenderContext" | "ToolRenderResultOptions"
        ) {
            let line = source::line(contents, pair[0].start);
            if owner != "maestro-tools" {
                return Err(format!(
                    "{}:{line}: {name} declaration belongs to maestro-tools, not {owner}",
                    path.display()
                ));
            }
            if !declarations.insert(name.to_owned()) {
                return Err(format!(
                    "{}:{line}: duplicate declaration {name}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}
