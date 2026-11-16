//! Isolated filesystem support for manifest and catalog tests.
use crate::{agents::Agent, definitions::ManifestCache, updates::remote_manifest_path};
use std::path::PathBuf;

pub(crate) struct Fixture {
    /// The disposable root that removes the tree.
    root: maestro_test_directory::Scratch,
    pub(crate) cache: ManifestCache,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        let root = maestro_test_directory::Scratch::new().unwrap();
        let cache = ManifestCache::new(
            Some(root.path().join("overrides")),
            Some(root.path().join("catalog")),
        )
        .unwrap();
        Self { root, cache }
    }
    /// The manifest override directory inside this fixture.
    pub(crate) fn overrides(&self) -> PathBuf {
        self.root.path().join("overrides")
    }
    /// The remote catalog directory inside this fixture.
    pub(crate) fn catalog(&self) -> PathBuf {
        self.root.path().join("catalog")
    }
    pub(crate) fn write_remote(&mut self, content: &str) {
        self.write_remote_without_reload(content);
        self.cache.reload_manifests().unwrap();
    }
    pub(crate) fn write_remote_without_reload(&self, content: &str) {
        let path = remote_manifest_path(&self.catalog(), Agent::Codex);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    pub(crate) fn write_local(&mut self, content: &str) {
        std::fs::create_dir_all(self.overrides()).unwrap();
        std::fs::write(self.overrides().join("codex.toml"), content).unwrap();
        self.cache.reload_manifests().unwrap();
    }
}
#[test]
fn screen_dispatch_exposes_live_working_metadata() {
    let fixture = Fixture::new();
    let detection = crate::detect_agent(&fixture.cache, Some(Agent::Pi), "Working...");
    assert_eq!(detection.state, crate::agents::AgentState::Working);
    assert!(detection.visible.visible_working);
}

#[test]
fn absent_screen_agent_has_unknown_state() {
    let fixture = Fixture::new();
    let detection = crate::detect_agent(&fixture.cache, None, "Working...");
    assert_eq!(detection.state, crate::agents::AgentState::Unknown);
    assert!(!detection.skip_state_update);
    assert_eq!(detection.visible, crate::agents::AgentVisibility::default());
    assert!(!crate::should_skip_state_update(
        &fixture.cache,
        None,
        "Working..."
    ));
}
