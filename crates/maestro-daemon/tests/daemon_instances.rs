//! Named-daemon deletion through real socket and filesystem owners.
#![cfg(all(test, unix))]

use maestro_daemon::{instances, paths::DaemonPaths};

#[test]
fn daemon_deletion_preserves_running_and_unrelated_folders() {
    let scratch = maestro_test_directory::Scratch::new_socket().unwrap();
    let root = scratch.path();
    let agent = root.to_str().unwrap();
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(root.join("keep"), "root data").unwrap();
    for name in ["idle", "live", "unrelated", "default"] {
        let path = root.join("daemons").join(name);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("keep"), name).unwrap();
    }
    let idle = instances::delete_session(agent, "idle").unwrap();
    assert_eq!(idle.name, "idle");
    assert!(!idle.default);
    assert!(!idle.running);
    assert!(!root.join("daemons/idle").exists());
    assert_eq!(
        instances::delete_session(agent, "missing").unwrap().name,
        "missing"
    );
    assert!(instances::delete_session(agent, "default").is_err());
    assert!(instances::delete_session(agent, "../escape").is_err());
    let socket = DaemonPaths::new(agent, Some("live"))
        .unwrap()
        .api_socket_path();
    let listener =
        maestro_ipc::local_socket::bind_local_listener(std::path::Path::new(&socket)).unwrap();
    assert_eq!(
        instances::delete_session(agent, "live").unwrap_err(),
        "daemon live is running; stop it before deleting"
    );
    for (path, content) in [
        ("keep", "root data"),
        ("daemons/unrelated/keep", "unrelated"),
        ("daemons/default/keep", "default"),
        ("daemons/live/keep", "live"),
    ] {
        assert_eq!(std::fs::read_to_string(root.join(path)).unwrap(), content);
    }
    drop(listener);
    std::fs::remove_file(socket).unwrap();
    assert_eq!(
        instances::delete_session(agent, "live").unwrap().name,
        "live"
    );
    std::fs::write(root.join("daemons/not-a-directory"), "preserved").unwrap();
    assert!(instances::delete_session(agent, "not-a-directory").is_err());
    assert_eq!(
        std::fs::read_to_string(root.join("daemons/not-a-directory")).unwrap(),
        "preserved"
    );
}
