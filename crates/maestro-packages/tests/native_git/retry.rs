//! Native checkout histories with controlled dependency failures.
use super::{Repository, git};
use crate::support::{Controlled, Effects};
use maestro_packages::{
    DefaultPackageManager, NativePackageOperations, PackageFuture, PackageManagerOptions,
    PackageOperations,
};
use maestro_settings::{Settings, SettingsManager};
use serde_json::json;
use std::{cell::RefCell, io, rc::Rc};

/// A live repository and its caller-owned execution context.
pub struct RetryFixture {
    /// Native history retained until all operations settle.
    pub repo: Repository,
    /// The manager under test.
    pub manager: Rc<DefaultPackageManager<Controlled>>,
    /// Live settings.
    pub settings: Rc<RefCell<SettingsManager>>,
    /// Command and capture observations.
    pub effects: Rc<Effects>,
    /// Command phase to reject once, or `dependencies` to reject repeatedly.
    pub fail: Rc<RefCell<String>>,
    /// Commit observed at each dependency operation.
    pub dependencies: Rc<RefCell<Vec<String>>>,
    /// Caller-driven local work owner.
    local: Rc<tokio::task::LocalSet>,
    /// Runtime that drives admitted children to completion.
    runtime: tokio::runtime::Runtime,
}
impl RetryFixture {
    /// Creates one of the four supported native repository layouts.
    pub fn new(layout: &str) -> Self {
        let mut repo = Repository::new();
        std::fs::write(repo.seed.join("package.json"), "{}").unwrap();
        std::fs::create_dir(repo.seed.join("extensions")).unwrap();
        std::fs::write(
            repo.seed.join("extensions/retained.ts"),
            "export const retained = true;",
        )
        .unwrap();
        git(&repo.seed, &["add", "package.json", "extensions"]);
        repo.commit("manifest");
        git(&repo.target, &["fetch", "origin"]);
        git(&repo.target, &["reset", "--hard", "origin/main"]);
        repo.original = git(&repo.target, &["rev-parse", "HEAD"]);
        configure_layout(&repo, layout);
        let root = repo.scratch.path();
        let settings = Rc::new(RefCell::new(SettingsManager::in_memory(Settings(
            json!({"packages":["git:github.com/acme/tool"]})
                .as_object()
                .unwrap()
                .clone(),
        ))));
        let local = Rc::new(tokio::task::LocalSet::new());
        let native = Rc::new(NativePackageOperations::new(Rc::new(|| false), &local));
        let effects = Rc::new(Effects::default());
        *effects.local.borrow_mut() = Rc::downgrade(&local);
        configure_capture(&effects, native.clone());
        let fail = Rc::new(RefCell::new(String::new()));
        let dependencies = Rc::new(RefCell::new(Vec::new()));
        configure_run(&effects, native, fail.clone(), dependencies.clone());
        let manager = Rc::new(DefaultPackageManager::new(
            PackageManagerOptions {
                cwd: root.join("cwd").to_string_lossy().into_owned(),
                agent_dir: root.join("agent").to_string_lossy().into_owned(),
                settings_manager: settings.clone(),
            },
            Controlled(effects.clone()),
        ));
        let runtime = crate::runtime::runtime();
        Self {
            repo,
            manager,
            settings,
            effects,
            fail,
            dependencies,
            local,
            runtime,
        }
    }
    /// Redirects only fresh clone transport to the fixture's actual local remote.
    pub fn use_local_clone(&self) {
        let remote = self.repo.remote.to_string_lossy().into_owned();
        let hook = self.effects.run_hook.borrow().clone().unwrap();
        *self.effects.run_hook.borrow_mut() = Some(Rc::new(move |command, args, cwd| {
            if let Some(index) = args.iter().position(|arg| arg == "clone") {
                let mut local = args.to_vec();
                local[index + 1].clone_from(&remote);
                return hook(command, &local, cwd);
            }
            hook(command, args, cwd)
        }));
    }
    /// Drives an operation to completion on its actual child runtime.
    pub fn run<T>(&self, future: PackageFuture<'_, T>) -> io::Result<T> {
        self.runtime.block_on(self.local.run_until(future))
    }
    /// Adds a manifest to both old and new commits before publishing a change.
    pub fn advance(&self) {
        std::fs::write(self.repo.seed.join("package.json"), "{}").unwrap();
        git(&self.repo.seed, &["add", "package.json"]);
        self.repo.commit("changed");
    }
}

/// Runs native Git while dependency preparation remains controlled and observable.
async fn execute(
    native: Rc<NativePackageOperations>,
    call: crate::sandbox::Call,
    fail: Rc<RefCell<String>>,
    observations: Rc<RefCell<Vec<String>>>,
) -> io::Result<Option<i32>> {
    if call.command != "git" {
        let cwd = std::path::Path::new(call.cwd.as_deref().unwrap());
        let oid = git(cwd, &["rev-parse", "HEAD"]);
        observations.borrow_mut().push(oid);
        if *fail.borrow() == "locked" {
            let lock = git(cwd, &["rev-parse", "--git-path", "index.lock"]);
            std::fs::write(cwd.join(lock), "held").unwrap();
        }
        if !fail.borrow().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "dependency rejected",
            ));
        }
        return Ok(Some(0));
    }
    let operation = if call.args[0] == "-c" {
        &call.args[2]
    } else {
        &call.args[0]
    };
    if *fail.borrow() == "selection-recovery" && operation == "symbolic-ref" {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "selection restoration rejected",
        ));
    }
    if fail.borrow().as_str() == operation {
        fail.borrow_mut().clear();
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "command rejected",
        ));
    }
    let result = native
        .run_command(&call.command, &call.args, call.cwd.as_deref())
        .await?;
    if *fail.borrow() == "after-checkout" && operation == "checkout" {
        fail.borrow_mut().clear();
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "checkout completion rejected",
        ));
    }
    Ok(result)
}

/// Prepares each repository layout through native Git.
fn configure_layout(repo: &Repository, layout: &str) {
    let root = repo.scratch.path();
    if layout == "linked" {
        let owner = root.join("owner");
        std::fs::rename(&repo.target, &owner).unwrap();
        git(
            &owner,
            &[
                "worktree",
                "add",
                "-b",
                "installed",
                repo.target.to_str().unwrap(),
                "main",
            ],
        );
        git(&repo.target, &["branch", "--set-upstream-to=origin/main"]);
        assert!(repo.target.join(".git").is_file());
        assert_eq!(
            std::fs::canonicalize(git(&repo.target, &["rev-parse", "--git-common-dir"])).unwrap(),
            std::fs::canonicalize(owner.join(".git")).unwrap()
        );
    } else if layout == "separate" {
        std::fs::remove_dir_all(&repo.target).unwrap();
        git(
            root,
            &[
                "clone",
                "--separate-git-dir",
                root.join("metadata").to_str().unwrap(),
                repo.remote.to_str().unwrap(),
                repo.target.to_str().unwrap(),
            ],
        );
        assert!(repo.target.join(".git").is_file());
        assert_eq!(
            std::fs::canonicalize(git(&repo.target, &["rev-parse", "--absolute-git-dir"])).unwrap(),
            std::fs::canonicalize(root.join("metadata")).unwrap()
        );
    } else if layout == "include" {
        let config = repo.target.join(".git/config");
        let text = std::fs::read_to_string(&config).unwrap();
        std::fs::write(repo.target.join(".git/included"), text).unwrap();
        std::fs::write(config, "[include]\npath = included\n").unwrap();
        assert_eq!(
            git(&repo.target, &["config", "--get", "include.path"]),
            "included"
        );
        assert_eq!(
            git(&repo.target, &["remote", "get-url", "origin"]),
            repo.remote.to_str().unwrap()
        );
    } else {
        assert_eq!(layout, "ordinary", "unsupported native repository layout");
        assert!(repo.target.join(".git").is_dir());
    }
}

/// Forwards captured commands through the native transport.
fn configure_capture(effects: &Rc<Effects>, native: Rc<NativePackageOperations>) {
    let capture = native;
    let observed = Rc::downgrade(effects);
    *effects.capture_hook.borrow_mut() = Some(Rc::new(move |command, args, cwd| {
        let (command, args, cwd) = (command.to_owned(), args.to_vec(), cwd.map(str::to_owned));
        let native = capture.clone();
        let env = observed
            .upgrade()
            .unwrap()
            .operands
            .borrow()
            .last()
            .unwrap()
            .2
            .clone();
        Box::pin(async move {
            let env: Vec<_> = env
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
                .collect();
            native
                .run_command_capture(
                    &command,
                    &args,
                    maestro_packages::CommandCaptureOptions {
                        cwd: cwd.as_deref(),
                        timeout: None,
                        env: &env,
                    },
                )
                .await
        })
    }));
}

/// Shares the existing controlled command seam across native callers.
fn configure_run(
    effects: &Effects,
    native: Rc<NativePackageOperations>,
    fail: Rc<RefCell<String>>,
    dependencies: Rc<RefCell<Vec<String>>>,
) {
    let failures = fail;
    let observations = dependencies;
    *effects.run_hook.borrow_mut() = Some(Rc::new(move |command, args, cwd| {
        let (command, args, cwd) = (command.to_owned(), args.to_vec(), cwd.map(str::to_owned));
        let (native, fail, observations) = (native.clone(), failures.clone(), observations.clone());
        Box::pin(execute(
            native,
            crate::sandbox::Call { command, args, cwd },
            fail,
            observations,
        ))
    }));
}

#[test]
#[should_panic(expected = "unsupported native repository layout")]
fn unknown_layout_does_not_silently_create_an_ordinary_clone() {
    RetryFixture::new("unknown");
}
