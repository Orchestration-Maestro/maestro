//! Producer-sealed group membership observed through public configured execution.
use super::{manager, types::Groups};
use maestro_packages::PackageManager;
use serde_json::json;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

/// Runs two scoped batches beside a four-worker Git group.
pub(super) fn run(user_jobs: usize, project_jobs: usize, git_jobs: usize) -> Groups {
    let user: Vec<_> = (0..user_jobs)
        .map(|i| format!("npm:u{i}"))
        .chain((0..git_jobs).map(|i| format!("git:github.com/acme/tool-{i}")))
        .collect();
    let project: Vec<_> = (0..project_jobs).map(|i| format!("npm:p{i}")).collect();
    let (manager, _, effects, tree) = manager(&json!(user), &json!(project));
    for i in 0..git_jobs {
        let checkout = format!("/fixture/agent/git/github.com/acme/tool-{i}");
        std::fs::create_dir_all(crate::support::below(tree.path(), &checkout)).unwrap();
    }
    *effects.capture_hook.borrow_mut() = Some(Rc::new(|_, _, _| {
        Box::pin(async { Ok("origin/main".into()) })
    }));
    let started = Rc::new(RefCell::new(BTreeMap::new()));
    let finished = Rc::new(RefCell::new(BTreeMap::new()));
    let (send, mut ready) = tokio::sync::mpsc::unbounded_channel();
    let (release, wait) = tokio::sync::watch::channel(false);
    install_groups(&effects, &started, &finished, send, wait);
    let local = Rc::new(tokio::task::LocalSet::new());
    *effects.local.borrow_mut() = Rc::downgrade(&local);
    let runtime = crate::runtime::runtime();
    let operation = local.spawn_local(manager.update(None));
    let first = runtime.block_on(local.run_until(async {
        for _ in 0..user_jobs.min(1) + project_jobs.min(1) + git_jobs.min(4) {
            ready.recv().await.unwrap();
        }
        let first = started.borrow().clone();
        release.send(true).unwrap();
        operation.await.unwrap().unwrap();
        first
    }));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
    let started = started.borrow().clone();
    let finished = finished.borrow().clone();
    Groups {
        first,
        started,
        finished,
    }
}

/// Records group admission and after-effect completion around the same release gate.
fn install_groups(
    effects: &crate::support::Effects,
    started: &Rc<RefCell<BTreeMap<String, bool>>>,
    finished: &Rc<RefCell<BTreeMap<String, bool>>>,
    send: tokio::sync::mpsc::UnboundedSender<()>,
    wait: tokio::sync::watch::Receiver<bool>,
) {
    let observed_starts = started.clone();
    let observed_finishes = finished.clone();
    *effects.run_hook.borrow_mut() = Some(Rc::new(move |program, args, cwd| {
        let label = if program == "git" {
            format!("acme/{}", cwd.unwrap().rsplit('/').next().unwrap())
        } else if args.iter().any(|arg| arg == "-g") {
            "user".into()
        } else {
            "project".into()
        };
        let send = send.clone();
        let starts = observed_starts.clone();
        let completions = observed_finishes.clone();
        let mut wait = wait.clone();
        Box::pin(async move {
            starts.borrow_mut().insert(label.clone(), true);
            send.send(()).unwrap();
            wait.wait_for(|ready| *ready).await.unwrap();
            completions.borrow_mut().insert(label, true);
            Ok(Some(0))
        })
    }));
}
