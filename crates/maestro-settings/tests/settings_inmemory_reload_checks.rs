mod support;
use maestro_settings::SettingsManager;
use serde_json::json;
use support::{Scheduler, block_on};
#[test]
fn in_memory_seed_survives_explicit_reload() {
    let q = Scheduler::default();
    let manager =
        SettingsManager::in_memory(json!({"theme":"dark","defaultModel":"seed"}), q.spawn())
            .unwrap();
    block_on(manager.reload());
    assert_eq!(manager.get_theme().as_deref(), Some("dark"));
    assert_eq!(manager.get_default_model().as_deref(), Some("seed"));
}

#[test]
fn theme_save_keeps_other_seeded_preferences() {
    let q = Scheduler::default();
    let manager = SettingsManager::in_memory(
        json!({"theme":"dark","defaultModel":"seed", "enabledModels":["a"],"custom":{"keep":true}}),
        q.spawn(),
    )
    .unwrap();
    manager.set_theme("light".into());
    q.drive();
    block_on(manager.flush());
    block_on(manager.reload());
    assert_eq!(manager.get_theme().as_deref(), Some("light"));
    assert_eq!(manager.get_default_model().as_deref(), Some("seed"));
    assert_eq!(
        manager.get_global_settings()["custom"],
        json!({"keep":true})
    );
    assert_eq!(manager.get_global_settings()["enabledModels"], json!(["a"]));
}

#[test]
fn in_memory_reload_matches_loader_owned_settings_calls() {
    let q = Scheduler::default();
    let manager =
        SettingsManager::in_memory(json!({"theme":"dark","defaultModel":"seed"}), q.spawn())
            .unwrap();
    for _ in 0..2 {
        block_on(manager.reload());
        assert_eq!(manager.get_theme().as_deref(), Some("dark"));
        assert_eq!(manager.get_default_model().as_deref(), Some("seed"));
    }
}
