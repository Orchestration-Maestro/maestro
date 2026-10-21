//! Native local Git refresh observed through the explicit resolver.
#![cfg(test)]
use super::super::{native_git_command, native_support, scratch, support};
use maestro_packages::{
    DefaultPackageManager, NativePackageOperations, PackageManager, PackageManagerOptions,
    PackageOperations, SourceScope,
};
use maestro_settings::SettingsListEntry;
use std::{cell::RefCell, path::Path, rc::Rc};
/// Constructs an executable through a child process instead of executing a writable file.
fn executable(root: &Path) -> String {
    let source = root.join("source");
    let target = root.join("package-command");
    std::fs::write(
        &source,
        "#!/bin/sh\nprintf 'installed\\n' >> dependency-log\n",
    )
    .unwrap();
    assert!(
        std::process::Command::new("install")
            .args(["-m", "0700"])
            .arg(source)
            .arg(&target)
            .status()
            .unwrap()
            .success()
    );
    target.to_string_lossy().into_owned()
}
/// Creates a local bare remote with a distinguishable resource.
fn repository(root: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let seed = root.join("seed");
    let remote = root.join("remote.git");
    std::fs::create_dir(&seed).unwrap();
    native_git_command::git(&seed, &["init", "-b", "main"]);
    std::fs::write(
        seed.join("package.json"),
        r#"{"maestro":{"extensions":["entry.ts"]}}"#,
    )
    .unwrap();
    native_support::write(seed.join("entry.ts").to_str().unwrap(), "first");
    native_git_command::git(&seed, &["add", "package.json", "entry.ts"]);
    native_git_command::git(&seed, &["commit", "-m", "first"]);
    native_git_command::git(
        root,
        &[
            "clone",
            "--bare",
            seed.to_str().unwrap(),
            remote.to_str().unwrap(),
        ],
    );
    native_git_command::git(
        &seed,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    (seed, remote)
}
/// Refreshes a real checkout and retains it after the origin becomes unavailable.
pub fn refresh() {
    let dir = scratch::Scratch::new().unwrap();
    let base = dir.path().to_string_lossy().into_owned();
    let (seed, remote) = repository(dir.path());
    let (_, settings, _) = support::manager(&serde_json::json!({}));
    settings
        .borrow_mut()
        .set_npm_command(Some(vec![SettingsListEntry::String(executable(
            dir.path(),
        ))]));
    let local = Rc::new(tokio::task::LocalSet::new());
    let native = Rc::new(NativePackageOperations::new(Rc::new(|| false), &local));
    let effects = Rc::new(support::Effects {
        native_exists: true,
        temp_root: RefCell::new(Some(format!("{base}/temp"))),
        ..Default::default()
    });
    let captured = Rc::clone(&native);
    *effects.capture_hook.borrow_mut() = Some(Rc::new(move |command, args, cwd| {
        let (command, args, cwd) = (command.to_owned(), args.to_vec(), cwd.map(str::to_owned));
        let native = Rc::clone(&captured);
        Box::pin(async move {
            native
                .run_command_capture(
                    &command,
                    &args,
                    maestro_packages::CommandCaptureOptions {
                        cwd: cwd.as_deref(),
                        timeout: Some(std::time::Duration::from_millis(10000)),
                        env: &[],
                    },
                )
                .await
        })
    }));
    let run = Rc::clone(&native);
    let origin = remote.to_string_lossy().into_owned();
    *effects.run_hook.borrow_mut() = Some(Rc::new(move |command, args, cwd| {
        let (command, mut args, cwd) = (command.to_owned(), args.to_vec(), cwd.map(str::to_owned));
        if args[0] == "clone" {
            args[1] = origin.clone();
        }
        let native = Rc::clone(&run);
        Box::pin(async move { native.run_command(&command, &args, cwd.as_deref()).await })
    }));
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: base.clone(),
            agent_dir: format!("{base}/user"),
            settings_manager: settings,
        },
        support::Controlled(effects),
    );
    crate::runtime::run(observe(&manager, &seed, &remote));
}
/// Assertions observe the same checkout before and after changed and failed refreshes.
async fn observe(manager: &DefaultPackageManager<support::Controlled>, seed: &Path, remote: &Path) {
    let sources = ["https://github.com/team/repo".into()];
    let first = manager
        .resolve_extension_sources(&sources, Some(SourceScope::Temporary))
        .await
        .unwrap();
    assert_eq!(first.extensions.len(), 1);
    let path = Path::new(&first.extensions[0].path);
    let target = path.parent().unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "first");
    assert_eq!(
        std::fs::read_to_string(target.join("dependency-log")).unwrap(),
        "installed\n"
    );
    std::fs::write(target.join("untracked"), "remove").unwrap();
    std::fs::write(seed.join("entry.ts"), "second").unwrap();
    native_git_command::git(seed, &["add", "entry.ts"]);
    native_git_command::git(seed, &["commit", "-m", "second"]);
    native_git_command::git(seed, &["push", "origin", "main"]);
    let second = manager
        .resolve_extension_sources(&sources, Some(SourceScope::Temporary))
        .await
        .unwrap();
    assert_eq!(second.extensions[0].path, first.extensions[0].path);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "second");
    assert!(!target.join("untracked").exists());
    // Clean removes the first dependency log before the second install runs.
    assert_eq!(
        std::fs::read_to_string(target.join("dependency-log")).unwrap(),
        "installed\n"
    );
    native_git_command::git(
        target,
        &[
            "remote",
            "set-url",
            "origin",
            remote.with_extension("missing").to_str().unwrap(),
        ],
    );
    let surviving = manager
        .resolve_extension_sources(&sources, Some(SourceScope::Temporary))
        .await
        .unwrap();
    assert_eq!(surviving.extensions[0].path, first.extensions[0].path);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "second");
}
