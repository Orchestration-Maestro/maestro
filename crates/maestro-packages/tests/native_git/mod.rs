//! Isolated native Git histories observed through configured update execution.
use maestro_packages::{
    DefaultPackageManager, NativePackageOperations, PackageManager, PackageManagerOptions,
};
use maestro_settings::{PackageSource, Settings, SettingsManager};
use std::{cell::RefCell, rc::Rc};

pub use super::native_git_command::git;
/// A retained remote, seed and installed checkout.
pub struct Repository {
    /// Owns the disposable directories.
    pub scratch: super::scratch::Scratch,
    /// Writable upstream history.
    pub seed: std::path::PathBuf,
    /// Bare origin used by the installed checkout.
    pub remote: std::path::PathBuf,
    /// Installed user contents.
    pub target: std::path::PathBuf,
    /// Initially installed commit.
    pub original: String,
}
impl Repository {
    /// Creates local repositories without network access.
    pub fn new() -> Self {
        let scratch = super::scratch::Scratch::new().unwrap();
        let seed = scratch.path().join("seed");
        let remote = scratch.path().join("remote.git");
        let target = scratch.path().join("agent/git/github.com/acme/tool");
        std::fs::create_dir(&seed).unwrap();
        git(&seed, &["init", "-b", "main"]);
        super::native_support::write(seed.join("content").to_str().unwrap(), "initial");
        git(&seed, &["add", "content"]);
        git(&seed, &["commit", "-m", "initial"]);
        git(
            scratch.path(),
            &[
                "clone",
                "--bare",
                seed.to_str().unwrap(),
                remote.to_str().unwrap(),
            ],
        );
        git(
            &seed,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        git(
            scratch.path(),
            &["clone", remote.to_str().unwrap(), target.to_str().unwrap()],
        );
        let original = git(&target, &["rev-parse", "HEAD"]);
        Self {
            scratch,
            seed,
            remote,
            target,
            original,
        }
    }
    /// Publishes a distinguishable commit to the local bare remote.
    pub fn commit(&self, text: &str) {
        std::fs::write(self.seed.join("content"), text).unwrap();
        git(&self.seed, &["add", "content"]);
        git(&self.seed, &["commit", "-m", text]);
        git(&self.seed, &["push", "--force", "origin", "main"]);
    }
    /// Updates by naming the configured user registration.
    pub fn update(&self, pinned: bool) {
        self.run_update(pinned, Some("git:github.com/acme/tool"));
    }
    /// Updates every configured registration, naming none.
    pub fn update_all(&self, pinned: bool) {
        self.run_update(pinned, None);
    }
    /// Updates the configured user registration on a caller-driven runtime.
    fn run_update(&self, pinned: bool, source: Option<&str>) {
        let settings = Rc::new(RefCell::new(
            SettingsManager::in_memory(Settings::default()),
        ));
        settings
            .borrow_mut()
            .set_packages(vec![PackageSource::Source(if pinned {
                "git:github.com/acme/tool@main".into()
            } else {
                "git:github.com/acme/tool".into()
            })]);
        // Unexpected dependency installation fails instead of reaching a registry.
        settings.borrow_mut().set_npm_command(Some(
            ["/bin/sh", "-c", "exit 97"]
                .map(|value| maestro_settings::SettingsListEntry::String(value.into()))
                .to_vec(),
        ));
        let local = Rc::new(tokio::task::LocalSet::new());
        let manager: Rc<dyn PackageManager> = Rc::new(DefaultPackageManager::new(
            PackageManagerOptions {
                cwd: self
                    .scratch
                    .path()
                    .join("cwd")
                    .to_string_lossy()
                    .into_owned(),
                agent_dir: self
                    .scratch
                    .path()
                    .join("agent")
                    .to_string_lossy()
                    .into_owned(),
                settings_manager: settings,
            },
            NativePackageOperations::new(Rc::new(|| false), &local),
        ));
        let runtime = crate::runtime::runtime();
        runtime
            .block_on(local.run_until(manager.update(source)))
            .unwrap();
        runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
    }
    /// Asserts actual head and content replacement, not merely child operands.
    pub fn assert_remote(&self, content: &str) {
        assert_eq!(
            git(&self.target, &["rev-parse", "HEAD"]),
            git(&self.remote, &["rev-parse", "HEAD"])
        );
        assert_ne!(git(&self.target, &["rev-parse", "HEAD"]), self.original);
        assert_eq!(
            std::fs::read_to_string(self.target.join("content")).unwrap(),
            content
        );
        assert!(
            !self
                .scratch
                .path()
                .join("cwd/.maestro/git/github.com/acme/tool")
                .exists()
        );
    }
}

/// Retry-safe native acquisition fixture.
pub mod retry;
