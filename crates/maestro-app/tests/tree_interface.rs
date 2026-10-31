//! Application exports used by tree navigation.
#![cfg(test)]
use maestro_app::presentation_data::keybindings::KEYBINDINGS;
use maestro_tui::{KeybindingDefinition, KeybindingKeys, KeybindingsManager, TUI_KEYBINDINGS};
use serde::Deserialize;
use std::collections::HashSet;

/// Recorded application action identities and filter definitions.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Definitions {
    /// Complete application action inventory.
    application_action_ids: HashSet<String>,
    /// Action identity and its definition.
    filters: Vec<(String, Definition)>,
}
/// Recorded scalar key and description.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Definition {
    /// Functional default key.
    default_keys: String,
    /// Action description.
    description: String,
}
/// The application table preserves toolkit defaults and registers tree actions.
#[test]
fn tree_filter_definitions_keep_toolkit_defaults() {
    let fixture: Definitions =
        serde_json::from_str(include_str!("fixtures/tree-filter-definitions.json")).unwrap();
    assert_eq!(
        &KEYBINDINGS[..TUI_KEYBINDINGS.len()],
        TUI_KEYBINDINGS.as_slice()
    );
    let mut ids = HashSet::new();
    for (id, _) in KEYBINDINGS.iter() {
        assert!(ids.insert(id), "Duplicate action id: {id}");
    }
    for (id, _) in &KEYBINDINGS[TUI_KEYBINDINGS.len()..] {
        assert!(
            fixture.application_action_ids.contains(id),
            "Unrecorded application action id: {id}"
        );
    }
    let manager = KeybindingsManager::new(KEYBINDINGS.clone(), Vec::new());
    for (id, row) in fixture.filters {
        assert_eq!(
            manager.get_definition(&id),
            Some(KeybindingDefinition {
                default_keys: KeybindingKeys::Single(row.default_keys.clone()),
                description: Some(row.description),
            })
        );
        assert_eq!(manager.get_keys(&id), vec![row.default_keys]);
    }
}

/// Supplies identity operands for the exported record views.
fn base() -> maestro_app::SessionEntryBase {
    let owner: maestro_session::session_manager::SessionEntryBase = maestro_app::SessionEntryBase {
        id: "entry".into(),
        parent_id: Some("parent".into()),
        timestamp: "2026-01-01T00:00:00Z".into(),
    };
    assert_eq!(owner.id, "entry");
    assert_eq!(owner.parent_id.as_deref(), Some("parent"));
    assert_eq!(owner.timestamp, "2026-01-01T00:00:00Z");
    owner
}

/// Checks a concrete owner-typed record against its complete serialized fields.
fn record<T: serde::Serialize>(value: T, kind: &str, mut fields: serde_json::Value) {
    fields["type"] = kind.into();
    fields["id"] = "entry".into();
    fields["parentId"] = "parent".into();
    fields["timestamp"] = "2026-01-01T00:00:00Z".into();
    assert_eq!(serde_json::to_value(value).unwrap(), fields);
}

/// Constructs an explicitly null optional field without a second presence type.
fn null<T: serde::de::DeserializeOwned>() -> T {
    serde_json::from_value(serde_json::Value::Null).unwrap()
}

/// Builds message and configuration records through the application export paths.
fn message_and_settings_records() {
    use maestro_app::{ModelChangeEntry, SessionMessageEntry, ThinkingLevelChangeEntry};
    use serde_json::json;
    let message: maestro_session::session_manager::SessionMessageEntry = SessionMessageEntry {
        base: base(),
        message: json!({"role":"user","content":"hello"}),
    };
    record(
        message,
        "message",
        json!({"message":{"role":"user","content":"hello"}}),
    );
    let thinking: maestro_session::session_manager::ThinkingLevelChangeEntry =
        ThinkingLevelChangeEntry {
            base: base(),
            thinking_level: "high".into(),
        };
    record(
        thinking,
        "thinking_level_change",
        json!({"thinkingLevel":"high"}),
    );
    let model: maestro_session::session_manager::ModelChangeEntry = ModelChangeEntry {
        base: base(),
        provider: "provider".into(),
        model_id: "model".into(),
    };
    record(
        model,
        "model_change",
        json!({"provider":"provider","modelId":"model"}),
    );
}

/// Builds label and session-info records through their existing owner.
fn label_and_info_records() {
    use maestro_app::{LabelEntry, SessionInfoEntry};
    use serde_json::json;
    let label: maestro_session::session_manager::LabelEntry = LabelEntry {
        base: base(),
        target_id: "target".into(),
        label: null(),
    };
    record(label, "label", json!({"targetId":"target","label":null}));
    let info: maestro_session::session_manager::SessionInfoEntry = SessionInfoEntry {
        base: base(),
        name: null(),
    };
    record(info, "session_info", json!({"name":null}));
}

/// Builds both summary record variants with distinguishable fields.
fn summary_records() {
    use maestro_app::{BranchSummaryEntry, CompactionEntry};
    use serde_json::json;
    let branch: maestro_session::session_manager::BranchSummaryEntry = BranchSummaryEntry {
        base: base(),
        from_id: "branch".into(),
        summary: "summary".into(),
        details: null(),
        from_hook: null(),
        usage: maestro_session::Presence::Missing,
    };
    record(
        branch,
        "branch_summary",
        json!({"fromId":"branch","summary":"summary","details":null,"fromHook":null}),
    );
    let compaction: maestro_session::session_manager::CompactionEntry = CompactionEntry {
        base: base(),
        summary: "compact".into(),
        first_kept_entry_id: "kept".into(),
        tokens_before: 12.0,
        details: null(),
        from_hook: null(),
        usage: maestro_session::Presence::Missing,
    };
    record(
        compaction,
        "compaction",
        json!({"summary":"compact","firstKeptEntryId":"kept","tokensBefore":12.0,"details":null,"fromHook":null}),
    );
}

/// Builds custom state and visible custom-message records.
fn custom_records() {
    use maestro_app::{CustomEntry, CustomMessageEntry};
    use serde_json::json;
    let custom: maestro_session::session_manager::CustomEntry = CustomEntry {
        base: base(),
        custom_type: "state".into(),
        data: null(),
    };
    record(custom, "custom", json!({"customType":"state","data":null}));
    let notice: maestro_session::session_manager::CustomMessageEntry = CustomMessageEntry {
        base: base(),
        custom_type: "notice".into(),
        content: serde_json::from_value(json!("text")).unwrap(),
        display: true,
        details: null(),
    };
    record(
        notice,
        "custom_message",
        json!({"customType":"notice","content":"text","display":true,"details":null}),
    );
}

/// Application records and scalar predicates retain their existing owner types.
#[test]
fn session_records_and_scalar_helpers_use_their_existing_owners() {
    message_and_settings_records();
    label_and_info_records();
    summary_records();
    custom_records();
    let entry: maestro_session::session_manager::SessionEntry =
        serde_json::json!({"id":"raw","open":{"nested":true}});
    let raw: &maestro_app::SessionEntry = &entry;
    let node: maestro_session::session_manager::SessionTreeNode<'_> =
        maestro_app::SessionTreeNode {
            entry: raw,
            children: Vec::new(),
            label: None,
            label_timestamp: None,
        };
    assert!(std::ptr::eq(node.entry, &raw const entry));
    assert_eq!(node.entry["open"]["nested"], true);
    assert!(maestro_app::is_whitespace_scalar('\u{feff}'));
    assert!(!maestro_app::is_whitespace_scalar('\u{85}'));
    assert!(maestro_app::is_truthy(&serde_json::json!([])));
    assert!(!maestro_app::is_truthy(&serde_json::json!(0)));
}
