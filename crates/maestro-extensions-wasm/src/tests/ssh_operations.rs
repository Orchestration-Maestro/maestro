//! Host-flavor path translation through the shipped operation callbacks.
use super::{bus_callbacks::captured_api, controlled::Controlled};
use crate::ExtensionResult;

#[path = "../../../../examples/extensions/ssh/operations.rs"]
mod operations;
#[path = "../../../../examples/extensions/ssh/transport.rs"]
mod transport;

#[test]
fn ssh_maps_windows_host_paths_to_posix_without_rewriting_other_roots() -> ExtensionResult<()> {
    let host = Controlled::default();
    host.observe().native_io.exit_code = Some(0);
    let remote = operations::Remote {
        api: captured_api(&host)?,
        host: "user@controlled".into(),
        cwd: "/srv".into(),
        local_cwd: r"C:\work\".into(),
    };
    let read = remote.options("read")?.read.operations.unwrap();
    for (path, expected) in [
        (r"C:\work\note.txt", "/srv/note.txt"),
        (r"C:\work\nested\note.txt", "/srv/nested/note.txt"),
        (r"C:\work", "/srv"),
        (r"C:\worker\note.txt", r"C:\worker\note.txt"),
        (r"D:\work\note.txt", r"D:\work\note.txt"),
    ] {
        super::block_on((read.read_file)(path.into())).unwrap();
        let observations = host.observed();
        let (_, args, _) = observations.native_io.launches.last().unwrap();
        assert_eq!(args[2], format!("cat -- {}", transport::quote(expected)));
    }
    let remote = operations::Remote {
        local_cwd: "C:/work/".into(),
        ..remote
    };
    let read = remote.options("read")?.read.operations.unwrap();
    super::block_on((read.read_file)(r"C:\work\note.txt".into())).unwrap();
    assert_eq!(
        host.observed().native_io.launches.last().unwrap().1[2],
        format!("cat -- {}", transport::quote("/srv/note.txt"))
    );
    // Backslashes remain literal file-name bytes for a POSIX host.
    let remote = operations::Remote {
        local_cwd: "/work".into(),
        ..remote
    };
    let read = remote.options("read")?.read.operations.unwrap();
    super::block_on((read.read_file)(r"/work/note\name".into())).unwrap();
    let observations = host.observed();
    assert_eq!(
        observations.native_io.launches.last().unwrap().1[2],
        "cat -- '/srv/note\\name'"
    );
    Ok(())
}
