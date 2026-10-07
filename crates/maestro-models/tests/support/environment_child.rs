use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Fixture {
    pub root: std::path::PathBuf,
}
impl Fixture {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "environment-fixture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        for part in ["home", "tmp", "config", "cache", "data", "state"] {
            std::fs::create_dir_all(root.join(part)).unwrap();
        }
        Self { root }
    }
    pub fn command(&self, test: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .env_clear()
            .current_dir(&self.root)
            .args(["--exact", test, "--nocapture"])
            .env("MAESTRO_ENV_CHILD", "1")
            .env("HOME", self.root.join("home"))
            .env("USERPROFILE", self.root.join("home"))
            .env("TMPDIR", self.root.join("tmp"))
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .env("XDG_CACHE_HOME", self.root.join("cache"))
            .env("XDG_DATA_HOME", self.root.join("data"))
            .env("XDG_STATE_HOME", self.root.join("state"));
        command
    }
    pub fn run(&self, test: &str, vars: &[(&str, &str)]) -> Output {
        let output = self
            .command(test)
            .envs(vars.iter().copied())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "stdout: {} stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}
pub fn child() -> bool {
    std::env::var_os("MAESTRO_ENV_CHILD").is_some()
}
