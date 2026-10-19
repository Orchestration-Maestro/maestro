//! Isolated filesystem support for manifest and catalog tests.
use crate::{agents::Agent, manifest::ManifestCache, manifest_update::remote_manifest_path};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

pub(crate) struct Fixture {
    pub(crate) root: PathBuf,
    pub(crate) overrides: PathBuf,
    pub(crate) catalog: PathBuf,
    pub(crate) cache: ManifestCache,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "maestro-detection-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let overrides = root.join("overrides");
        let catalog = root.join("catalog");
        let cache = ManifestCache::new(Some(overrides.clone()), Some(catalog.clone())).unwrap();
        Self {
            root,
            overrides,
            catalog,
            cache,
        }
    }
    pub(crate) fn write_remote(&mut self, content: &str) {
        self.write_remote_without_reload(content);
        self.cache.reload_manifests().unwrap();
    }
    pub(crate) fn write_remote_without_reload(&self, content: &str) {
        let path = remote_manifest_path(&self.catalog, Agent::Codex);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    pub(crate) fn write_local(&mut self, content: &str) {
        std::fs::create_dir_all(&self.overrides).unwrap();
        std::fs::write(self.overrides.join("codex.toml"), content).unwrap();
        self.cache.reload_manifests().unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
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
