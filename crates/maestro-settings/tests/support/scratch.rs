use super::{SettingsLocations, SettingsScope, Value};

pub(crate) struct Scratch {
    pub(crate) root: std::path::PathBuf,
}
impl Scratch {
    pub(crate) fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        loop {
            let root = std::env::temp_dir().join(format!(
                "maestro-settings-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            match std::fs::create_dir(&root) {
                Ok(()) => return Self { root },
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("create scratch: {e}"),
            }
        }
    }
    pub(crate) fn locations(&self) -> SettingsLocations {
        SettingsLocations::new(
            self.root.join("cwd"),
            None,
            self.root.join("user"),
            self.root.join("home"),
        )
        .unwrap()
    }
    pub(crate) fn write(&self, scope: SettingsScope, value: &Value) {
        let dir = self.locations().configuration_directory(scope);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("settings.json"),
            serde_json::to_vec(value).unwrap(),
        )
        .unwrap();
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let result = std::fs::remove_dir_all(&self.root);
        if !std::thread::panicking() {
            result.expect("remove owned scratch");
        }
    }
}
