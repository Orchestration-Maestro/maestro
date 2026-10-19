//! Checked workspace-default guide.

use crate::settings::{DEFAULTS_TEXT, io::project_global_settings, model::Config};
use maestro_settings::Settings;
use std::path::Path;

#[test]
fn workspace_defaults_match_the_settings_guide() {
    let text = DEFAULTS_TEXT
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let raw: Settings = serde_json::from_str(&text).unwrap();
    let loaded = project_global_settings(&raw, Path::new("."));
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    assert!(loaded.invalid_sections.is_empty());
    assert_eq!(loaded.config, Config::default());
    let guide = include_str!("../../../../../docs/settings.md");
    let block = guide
        .split_once("<!-- workspace-defaults:start -->\n```jsonc\n")
        .unwrap()
        .1
        .split_once("```\n<!-- workspace-defaults:end -->")
        .unwrap()
        .0;
    assert_eq!(block, DEFAULTS_TEXT);
}
