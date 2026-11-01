//! Current-model identity through retained shared descriptors.
use super::*;

/// Builds a typed descriptor without consulting the catalog.
fn descriptor(provider: &str, id: &str, name: &str) -> Arc<RwLock<Model>> {
    Arc::new(RwLock::new(
        serde_json::from_value(serde_json::json!({
            "provider": provider, "id": id, "name": name,
            "api": "openai-completions", "baseUrl": "https://example.invalid",
            "reasoning": false, "input": ["text"],
            "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0},
            "contextWindow": 1000, "maxTokens": 100
        }))
        .unwrap(),
    ))
}

#[test]
fn shared_current_comparison_uses_handle_identity_without_reading() {
    let model = descriptor("alpha", "a1", "Name");
    let item = ModelItem::new(model.clone());
    let guard = model.write().unwrap();
    eprintln!("shared descriptor write retained; entering comparison");
    assert!(item.is_current(Some(&model)));
    drop(guard);
}

#[test]
fn distinct_current_descriptors_use_live_provider_and_id() {
    let model = descriptor("alpha", "a1", "Name");
    let item = ModelItem::new(model.clone());
    // Absent and null input both become None at the typed boundary.
    assert!(!item.is_current(None));
    for (provider, id, other_provider, other_id, expected) in [
        ("alpha", "a1", "alpha", "a1", true),
        ("alpha", "a1", "beta", "a1", false),
        ("alpha", "a1", "alpha", "a2", false),
        ("alpha", "a1", "Alpha", "a1", false),
        ("alpha", "a1", "alpha", "A1", false),
        ("", "", "", "", true),
        ("提供者", "é", "提供者", "é", true),
        ("alpha", "é", "alpha", "e\u{301}", false),
    ] {
        let item = ModelItem::new(descriptor(provider, id, "Name"));
        let current = descriptor(other_provider, other_id, "Other");
        assert_eq!(item.is_current(Some(&current)), expected);
    }
    let current = descriptor("alpha", "a1", "Name");
    let mut observed = vec![item.is_current(Some(&current))];
    for (target, provider, id, name) in [
        (&current, Some("beta"), None, None),
        (&current, Some("alpha"), Some("a2"), None),
        (&current, None, Some("a1"), Some("Edited")),
        (&model, Some("gamma"), None, None),
        (&current, Some("gamma"), None, None),
        (&model, None, Some("a3"), None),
        (&current, None, Some("a3"), None),
    ] {
        {
            let mut value = target.write().unwrap();
            if let Some(provider) = provider {
                value.provider = provider.into();
            }
            if let Some(id) = id {
                value.id = id.into();
            }
            if let Some(name) = name {
                value.name = name.into();
            }
        }
        observed.push(item.is_current(Some(&current)));
    }
    assert_eq!(
        observed,
        [true, false, false, true, false, true, false, true]
    );
}
