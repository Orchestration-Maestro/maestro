//! The package filter list for browser contribution files.
#![cfg(test)]
use crate::support::{block_on, manager_with, raw_json, text_entries};
use maestro_settings::{PackageSource, SettingsListEntry, SettingsScope};
use serde_json::json;

/// A package's `contributions` list reads, writes and keeps its members like the other four.
#[test]
fn contribution_paths_are_read_beside_the_other_lists() {
    let stored = json!([
        {"source": "npm:views", "contributions": ["pages/*.json", {"opaque": 1}], "themes": ["t"]},
        {"source": "npm:plain"}
    ]);
    let (mut settings, storage) = manager_with(json!({"packages": stored}));
    let mut packages = settings.get_packages();
    let PackageSource::Filtered(views) = &mut packages[0] else {
        panic!("filtered entry expected, got {:?}", packages[0]);
    };
    assert_eq!(
        views.contributions(),
        Some(vec![
            SettingsListEntry::String("pages/*.json".into()),
            SettingsListEntry::Unknown(json!({"opaque": 1})),
        ])
    );
    assert_eq!(views.themes(), Some(text_entries(&["t"])));
    views.set_contributions(Some(text_entries(&["cards/home.json"])));
    assert!(
        matches!(&packages[1], PackageSource::Filtered(plain) if plain.contributions().is_none())
    );
    settings.set_packages(packages);
    block_on(settings.flush());
    assert_eq!(
        raw_json(&*storage, SettingsScope::Global),
        json!({"packages": [
            {"source": "npm:views", "contributions": ["cards/home.json"], "themes": ["t"]},
            {"source": "npm:plain"}
        ]})
    );
}

/// An object whose `contributions` is an array is a filter; any other value keeps it opaque.
#[test]
fn filtered_object_lists_keep_contribution_arrays() {
    let (settings, _) = manager_with(json!({"packages": [
        {"source": "npm:views", "contributions": []},
        {"source": "npm:bad", "contributions": "pages"},
        {"source": "npm:null", "contributions": null}
    ]}));
    let packages = settings.get_packages();
    assert!(
        matches!(&packages[0], PackageSource::Filtered(views) if views.contributions() == Some(Vec::new()))
    );
    assert!(
        matches!(&packages[1], PackageSource::Unknown(value) if value["contributions"] == "pages")
    );
    assert!(matches!(&packages[2], PackageSource::Unknown(_)));
}
