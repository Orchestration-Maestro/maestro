//! Single-test child processes over a disposable agent directory, shared by the tests
//! inside the library and the tests that drive the interactive mode from outside.
use maestro_app::configuration::config::ENV_AGENT_DIR;
use maestro_test_process::{IsolatedSetup, SearchGuard, TestProcess};
use std::{ffi::OsStr, path::Path};

/// Runs `body` in a fresh offline single-test child whose agent directory is a disposable
/// root, so the constructor reads its key bindings from there rather than the developer's.
pub(crate) fn isolated(test: &str, body: impl AsyncFnOnce(&Path)) {
    isolated_case(
        test,
        "child",
        &[(OsStr::new("MAESTRO_OFFLINE"), OsStr::new("1"))],
        body,
    );
}

/// Installs stub `fd` and `rg`, which answer at once with nothing, in the binary directory,
/// which is searched before the command path.
pub(crate) fn controlled_search_programs(directory: &Path) {
    maestro_test_process::install_stub_search_programs(&directory.join("bin"))
        .expect("the stub search programs install");
}

/// Guards `path`, the command path of a child over `root`, with the shared search guard.
pub(crate) fn search_guard(root: &Path, path: &OsStr) -> SearchGuard {
    SearchGuard::new(root, path).expect("the search guard installs")
}

/// Runs `body` as `case` of `test` in a fresh single-test child over a disposable agent
/// directory, with no inherited environment apart from the command path and `envs`, so
/// no developer credential reaches it. The child's temporary directory is that root's
/// `tmp`, its commands are looked up in the root's `bin` first, and its clipboard is a
/// Wayland one. Unless `envs` supplies the command path, the root's `bin` holds both
/// search programs; a [`SearchGuard`] fails the run if the child ran a host copy. The
/// file-size signal keeps the child alive, so a write over a lowered limit fails instead
/// of ending the child. A prepared native fixture stays available to it.
pub(crate) fn isolated_case(
    test: &str,
    case: &str,
    envs: &[(&OsStr, &OsStr)],
    body: impl AsyncFnOnce(&Path),
) {
    if let Some(output) = isolated_output(
        test,
        case,
        envs,
        IsolatedSetup::new().file_size_continues(),
        body,
    ) {
        assert!(
            output.status.success(),
            "child probe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Runs `body` as `case` of `test` in the child [`isolated_case`] describes, after `setup`
/// is applied at the child's entry, and returns that child's output to the parent; the
/// child itself returns nothing.
pub(crate) fn isolated_output(
    test: &str,
    case: &str,
    envs: &[(&OsStr, &OsStr)],
    setup: IsolatedSetup,
    body: impl AsyncFnOnce(&Path),
) -> Option<std::process::Output> {
    let (output, guard) = guarded_output(test, case, envs, setup, body)?;
    guard.assert_unreached();
    Some(output)
}

/// Runs the child [`isolated_output`] describes and returns its output beside the guard on
/// its command path.
pub(crate) fn guarded_output(
    test: &str,
    case: &str,
    envs: &[(&OsStr, &OsStr)],
    setup: IsolatedSetup,
    body: impl AsyncFnOnce(&Path),
) -> Option<(std::process::Output, SearchGuard)> {
    let selection = maestro_test_process::selection();
    if selection.is_child() {
        let _setup = IsolatedSetup::apply_requested().expect("the child applies its request");
        if selection.runs_case(case) {
            let directory = std::env::var(&*ENV_AGENT_DIR).unwrap();
            maestro_test_process::executor::run(body(Path::new(&directory)));
        }
        return None;
    }
    let scratch = maestro_test_directory::Scratch::new().expect("scratch directory is new");
    let root = scratch.path();
    for name in ["bin", "tmp"] {
        std::fs::create_dir(root.join(name)).unwrap();
    }
    let supplied = envs.iter().rev().find(|(key, _)| *key == "PATH");
    let guard = if let Some((_, path)) = supplied {
        search_guard(root, path)
    } else {
        controlled_search_programs(root);
        let path = format!(
            "{}:{}",
            root.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        search_guard(root, OsStr::new(&path))
    };
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE");
    let mut all: Vec<(&OsStr, &OsStr)> = vec![
        (OsStr::new(ENV_AGENT_DIR.as_str()), root.as_os_str()),
        (OsStr::new("WAYLAND_DISPLAY"), OsStr::new("maestro-test")),
    ];
    // The cleared child installs the controlled programs from the prepared artifact.
    all.extend(
        artifact
            .iter()
            .map(|artifact| (OsStr::new("MAESTRO_NATIVE_FIXTURE"), artifact.as_os_str())),
    );
    all.extend(envs.iter().copied());
    all.push((OsStr::new("PATH"), guard.path()));
    let output = TestProcess::current(test)
        .unwrap()
        .env_clear()
        .case(case)
        .setup(setup)
        .temp_dir(root.join("tmp"))
        .nocapture()
        .single_threaded()
        .current_dir(root)
        .envs(all)
        .output()
        .expect("child starts");
    Some((output, guard))
}
