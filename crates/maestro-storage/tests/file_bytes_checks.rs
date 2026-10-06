mod support;
use maestro_storage::conformance;

#[test]
fn explicit_paths_and_recursive_directories() {
    support::both(
        "paths",
        conformance::explicit_paths_and_recursive_directories,
    );
    use maestro_storage::Storage;
    let root = std::path::Path::new("controlled-links");
    let storage = support::Controlled::new(root);
    let file = root.join("file");
    storage.write_file(&file, b"target").unwrap();
    storage.link(&root.join("file-link"), &file);
    storage.link(&root.join("dir-link"), root);
    storage.link(&root.join("dangling"), &root.join("missing"));
    assert!(storage.exists(&root.join("file-link")));
    assert!(storage.exists(&root.join("dir-link")));
    assert!(!storage.exists(&root.join("dangling")));
}

#[test]
fn empty_paths_are_not_found_for_every_operation() {
    use maestro_storage::Storage;
    use std::{io::ErrorKind, path::Path};

    let mut outcomes = Vec::new();
    for relative in [false, true] {
        let fixture = support::Fixture::new("empty-path", relative);
        let controlled = support::Controlled::new(&fixture.root);
        let adapters: &[&dyn Storage] = &[
            &controlled,
            #[cfg(not(target_arch = "wasm32"))]
            &maestro_storage::FileStorage,
        ];
        for storage in adapters {
            let path = Path::new("");
            let errors = [
                ("mkdir", storage.mkdir(path).err().map(|e| e.kind())),
                ("read", storage.read_file(path).err().map(|e| e.kind())),
                (
                    "prefix",
                    storage.read_prefix(path, 512).err().map(|e| e.kind()),
                ),
                (
                    "zero prefix",
                    storage.read_prefix(path, 0).err().map(|e| e.kind()),
                ),
                ("mtime", storage.modified(path).err().map(|e| e.kind())),
                ("readdir", storage.read_dir(path).err().map(|e| e.kind())),
                (
                    "append",
                    storage.append_file(path, b"").err().map(|e| e.kind()),
                ),
                (
                    "write",
                    storage.write_file(path, b"").err().map(|e| e.kind()),
                ),
                (
                    "async read",
                    support::ready(storage.read_file_async(path))
                        .err()
                        .map(|e| e.kind()),
                ),
                (
                    "async mtime",
                    support::ready(storage.modified_async(path))
                        .err()
                        .map(|e| e.kind()),
                ),
                (
                    "async readdir",
                    support::ready(storage.read_dir_async(path))
                        .err()
                        .map(|e| e.kind()),
                ),
                (
                    "async dirents",
                    support::ready(storage.read_dir_with_file_types_async(path))
                        .err()
                        .map(|e| e.kind()),
                ),
            ];
            outcomes.push((errors, storage.exists(path)));
        }
    }
    for (errors, exists) in outcomes {
        for (operation, kind) in errors {
            assert_eq!(kind, Some(ErrorKind::NotFound), "empty path: {operation}");
        }
        assert!(!exists, "empty path must not exist");
    }
    let controlled = support::Controlled::default();
    controlled.mkdir(Path::new("relative/child")).unwrap();
    assert!(controlled.exists(Path::new("relative")));
    assert!(controlled.exists(Path::new("relative/child")));
}

#[test]
fn opaque_file_reads() {
    support::both("opaque", conformance::opaque_file_reads);
}

#[test]
fn prefix_reads_keep_bytes_and_requested_bound() {
    support::both(
        "prefix",
        conformance::prefix_reads_keep_bytes_and_requested_bound,
    );
}

#[test]
fn append_chunks_without_framing() {
    support::both("append", conformance::append_chunks_without_framing);
}

#[test]
fn rewrite_removes_old_suffix() {
    support::both("rewrite", conformance::rewrite_removes_old_suffix);
}

#[test]
fn directory_names_and_modification_time_are_separate() {
    support::both(
        "names",
        conformance::directory_names_and_modification_time_are_separate,
    );
    use maestro_storage::Storage;
    let expected = std::time::UNIX_EPOCH + std::time::Duration::from_secs(123);
    let root = std::path::Path::new("controlled-metadata");
    let controlled = support::Controlled::new(root);
    controlled.write_file(&root.join("file"), b"raw").unwrap();
    assert_eq!(controlled.modified(&root.join("file")).unwrap(), expected);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let fixture = support::Fixture::new("metadata", false);
        let file = fixture.root.join("file");
        maestro_storage::FileStorage
            .write_file(&file, b"raw")
            .unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&file)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(expected))
            .unwrap();
        assert_eq!(
            maestro_storage::FileStorage.modified(&file).unwrap(),
            expected
        );
    }
}

#[test]
fn io_failure_leaves_later_operations_available() {
    support::both(
        "failure",
        conformance::io_failure_leaves_later_operations_available,
    );
}

#[test]
fn same_byte_caller_accepts_each_adapter() {
    support::both("caller", conformance::same_byte_caller_accepts_each_adapter);
}

#[cfg(target_os = "linux")]
#[test]
fn failed_prefix_read_retains_open_descriptor() {
    use maestro_storage::{FileStorage, Storage};
    if support::isolated_child(
        "MAESTRO_PREFIX_CHILD",
        "failed_prefix_read_retains_open_descriptor",
    ) {
        return;
    }
    let fixture = support::Fixture::new("leak", false);
    let count = || std::fs::read_dir("/proc/self/fd").unwrap().count();
    let baseline = count();
    assert_eq!(
        FileStorage
            .read_prefix(&fixture.root.join("absent"), 1)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::NotFound
    );
    assert_eq!(count(), baseline);
    let file = fixture.root.join("ok");
    FileStorage.write_file(&file, b"ok").unwrap();
    assert_eq!(FileStorage.read_prefix(&file, 1).unwrap(), b"o");
    assert_eq!(count(), baseline);
    assert!(
        FileStorage
            .read_prefix(&fixture.root, 0)
            .unwrap()
            .is_empty()
    );
    assert_eq!(count(), baseline);
    assert_eq!(
        FileStorage
            .read_prefix(&fixture.root, 1)
            .unwrap_err()
            .raw_os_error(),
        Some(21)
    );
    assert_eq!(count(), baseline + 1);
}

#[test]
fn partial_failure_does_not_poison_byte_access() {
    use maestro_storage::Storage;
    let root = std::path::Path::new("controlled-partial");
    let storage = support::Controlled::new(root);
    let file = root.join("file");
    storage.write_file(&file, b"old").unwrap();
    storage.fail_after(2);
    assert_eq!(
        storage
            .append_file(&file, b"ABCD")
            .unwrap_err()
            .raw_os_error(),
        Some(28)
    );
    assert_eq!(storage.read_file(&file).unwrap(), b"oldAB");
    storage.append_file(&file, b"!").unwrap();
    assert_eq!(storage.read_file(&file).unwrap(), b"oldAB!");
    storage.fail_after(1);
    assert_eq!(
        storage
            .write_file(&file, b"XYZ")
            .unwrap_err()
            .raw_os_error(),
        Some(28)
    );
    assert_eq!(storage.read_file(&file).unwrap(), b"X");
    storage.append_file(&file, b"!").unwrap();
    assert_eq!(storage.read_file(&file).unwrap(), b"X!");
}

#[test]
fn async_operations_keep_raw_results() {
    fn assertion(storage: &dyn maestro_storage::Storage, root: &std::path::Path) {
        let file = root.join("async-bytes");
        storage
            .write_file(&file, b"\0\xff\r\n\xef\xbb\xbf\xc2\x85")
            .unwrap();
        assert_eq!(
            support::ready(storage.read_file_async(&file)).unwrap(),
            b"\0\xff\r\n\xef\xbb\xbf\xc2\x85"
        );
        assert_eq!(
            support::ready(storage.modified_async(&file)).unwrap(),
            storage.modified(&file).unwrap()
        );
        assert_eq!(
            support::ready(storage.read_dir_async(root)).unwrap(),
            storage.read_dir(root).unwrap()
        );
        let missing = root.join("missing-async");
        assert_eq!(
            support::ready(storage.read_file_async(&missing))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::NotFound
        );
        assert!(support::ready(storage.modified_async(&missing)).is_err());
        assert!(support::ready(storage.read_dir_async(&missing)).is_err());
        assert!(support::ready(storage.read_dir_async(&file)).is_err());
        storage.append_file(&file, b"!").unwrap();
        assert_eq!(
            support::ready(storage.read_file_async(&file)).unwrap(),
            b"\0\xff\r\n\xef\xbb\xbf\xc2\x85!"
        );
    }
    support::both("async", assertion);
}

#[test]
fn directory_entry_kinds_do_not_follow_links() {
    use maestro_storage::{Dirent, DirentKind, Storage};
    let root = std::path::Path::new("controlled-kinds");
    let controlled = support::Controlled::new(root);
    let entries = vec![
        Dirent {
            name: "z-file".into(),
            kind: DirentKind::File,
        },
        Dirent {
            name: "a-dir".into(),
            kind: DirentKind::Directory,
        },
        Dirent {
            name: "link".into(),
            kind: DirentKind::SymbolicLink,
        },
        Dirent {
            name: "other".into(),
            kind: DirentKind::Other,
        },
    ];
    controlled.entries(root, entries.clone());
    assert_eq!(
        support::ready(controlled.read_dir_with_file_types_async(root)).unwrap(),
        entries
    );
    assert_eq!(
        controlled.read_dir(root).unwrap(),
        ["z-file", "a-dir", "link", "other"].map(std::ffi::OsString::from)
    );
    assert!(controlled.modified(&root.join("z-file")).is_err());
    assert!(
        support::ready(controlled.read_dir_with_file_types_async(&root.join("missing"))).is_err()
    );
    #[cfg(target_os = "linux")]
    {
        use maestro_storage::FileStorage;
        use std::os::unix::{fs::symlink, net::UnixListener};
        let fixture = support::Fixture::new("kinds", false);
        let root = &fixture.root;
        FileStorage.write_file(&root.join("file"), b"raw").unwrap();
        FileStorage.mkdir(&root.join("dir")).unwrap();
        symlink("file", root.join("file-link")).unwrap();
        symlink("dir", root.join("dir-link")).unwrap();
        symlink("absent", root.join("dangling")).unwrap();
        let _socket = UnixListener::bind(root.join("socket")).unwrap();
        assert_eq!(
            support::ready(FileStorage.read_dir_with_file_types_async(root)).unwrap(),
            vec![
                Dirent {
                    name: "dangling".into(),
                    kind: DirentKind::SymbolicLink
                },
                Dirent {
                    name: "dir".into(),
                    kind: DirentKind::Directory
                },
                Dirent {
                    name: "dir-link".into(),
                    kind: DirentKind::SymbolicLink
                },
                Dirent {
                    name: "file".into(),
                    kind: DirentKind::File
                },
                Dirent {
                    name: "file-link".into(),
                    kind: DirentKind::SymbolicLink
                },
                Dirent {
                    name: "socket".into(),
                    kind: DirentKind::Other
                },
            ]
        );
        assert!(FileStorage.exists(&root.join("file-link")));
        assert!(FileStorage.exists(&root.join("dir-link")));
        assert!(!FileStorage.exists(&root.join("dangling")));
        assert!(
            support::ready(FileStorage.read_dir_with_file_types_async(&root.join("absent")))
                .is_err()
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn native_write_keeps_link_target_identity() {
    use maestro_storage::{DirentKind, FileStorage, Storage};
    use std::os::unix::fs::{MetadataExt, symlink};
    let fixture = support::Fixture::new("identity", false);
    let target = fixture.root.join("target");
    let link = fixture.root.join("link");
    let alias = fixture.root.join("alias");
    FileStorage.write_file(&target, b"original suffix").unwrap();
    symlink("target", &link).unwrap();
    std::fs::hard_link(&target, &alias).unwrap();
    let identity = std::fs::metadata(&target).unwrap();
    FileStorage.write_file(&link, b"new").unwrap();
    FileStorage.append_file(&link, b"!").unwrap();
    let after = std::fs::metadata(&target).unwrap();
    assert_eq!((after.dev(), after.ino()), (identity.dev(), identity.ino()));
    assert_eq!(
        std::fs::read_link(&link).unwrap(),
        std::path::Path::new("target")
    );
    assert_eq!(FileStorage.read_file(&alias).unwrap(), b"new!");
    assert_eq!(FileStorage.read_file(&link).unwrap(), b"new!");
    assert!(FileStorage.exists(&link));
    assert_eq!(
        FileStorage.modified(&link).unwrap(),
        FileStorage.modified(&target).unwrap()
    );
    assert_eq!(FileStorage.read_prefix(&link, 2).unwrap(), b"ne");
    symlink("missing", fixture.root.join("dangling")).unwrap();
    assert!(!FileStorage.exists(&fixture.root.join("dangling")));
    assert!(
        FileStorage
            .modified(&fixture.root.join("dangling"))
            .is_err()
    );
    let entries =
        support::ready(FileStorage.read_dir_with_file_types_async(&fixture.root)).unwrap();
    assert_eq!(
        entries.iter().find(|e| e.name == "link").unwrap().kind,
        DirentKind::SymbolicLink
    );
    assert_eq!(
        entries.iter().find(|e| e.name == "dangling").unwrap().kind,
        DirentKind::SymbolicLink
    );
}

#[test]
fn async_results_are_observable_in_completion_order() {
    use maestro_storage::Storage;
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Context, Poll, Wake, Waker},
    };
    struct Count(AtomicUsize);
    impl Wake for Count {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let root = std::path::Path::new("controlled-pending");
    let storage = support::Controlled::new(root);
    let file = root.join("file");
    storage.write_file(&file, b"admitted").unwrap();
    storage.hold();
    let mut read = storage.read_file_async(&file);
    let mut stat = storage.modified_async(&root.join("missing"));
    let mut names = storage.read_dir_async(root);
    let mut kinds = storage.read_dir_with_file_types_async(root);
    assert_eq!(storage.admitted(), 4);
    let first = Arc::new(Count(AtomicUsize::new(0)));
    let second = Arc::new(Count(AtomicUsize::new(0)));
    let w1 = Waker::from(first.clone());
    let w2 = Waker::from(second.clone());
    let mut cx1 = Context::from_waker(&w1);
    let mut cx2 = Context::from_waker(&w2);
    assert!(read.as_mut().poll(&mut cx1).is_pending());
    assert!(names.as_mut().poll(&mut cx1).is_pending());
    assert!(names.as_mut().poll(&mut cx1).is_pending());
    assert!(names.as_mut().poll(&mut cx2).is_pending());
    storage.release(2);
    assert_eq!(first.0.load(Ordering::SeqCst), 0);
    assert_eq!(second.0.load(Ordering::SeqCst), 1);
    match names.as_mut().poll(&mut cx2) {
        Poll::Ready(Ok(names)) => assert_eq!(names, vec![std::ffi::OsString::from("file")]),
        _ => panic!("names not completed"),
    }
    assert!(read.as_mut().poll(&mut cx1).is_pending());
    assert!(stat.as_mut().poll(&mut cx2).is_pending());
    storage.release(1);
    match stat.as_mut().poll(&mut cx2) {
        Poll::Ready(Err(error)) => assert_eq!(error.kind(), std::io::ErrorKind::NotFound),
        other => panic!("expected selected stat failure: {other:?}"),
    }
    storage.release(3);
    assert!(kinds.as_mut().poll(&mut cx1).is_ready());
    assert!(read.as_mut().poll(&mut cx1).is_pending());
    storage.release(0);
    match read.as_mut().poll(&mut cx1) {
        Poll::Ready(Ok(bytes)) => assert_eq!(bytes, b"admitted"),
        _ => panic!("read not completed"),
    }
    let dropped = storage.read_file_async(&file);
    drop(dropped);
    storage.release(4);
    assert_eq!(storage.completed(), 5);
    let early = storage.read_file_async(&file);
    storage.release(5);
    assert_eq!(support::ready(early).unwrap(), b"admitted");
    assert_eq!(storage.completed(), 6);
}

#[cfg(target_os = "linux")]
#[test]
fn native_async_read_does_not_block_other_requests() {
    use maestro_storage::{FileStorage, Storage};
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Context, Waker},
    };
    if support::isolated_child(
        "MAESTRO_FIFO_CHILD",
        "native_async_read_does_not_block_other_requests",
    ) {
        return;
    }
    use support::{Notify, Release};
    let fixture = support::Fixture::new("fifo", false);
    let fifo = fixture.root.join("fifo");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let mut read = FileStorage.read_file_async(&fifo);
    // The writer open rendezvous proves the worker has opened the read end.
    let mut release = Release(Some(
        std::fs::OpenOptions::new().write(true).open(&fifo).unwrap(),
    ));
    let (tx1, rx1) = std::sync::mpsc::channel();
    let (tx2, rx2) = std::sync::mpsc::channel();
    let first = Arc::new(Notify {
        wakes: AtomicUsize::new(0),
        tx: tx1,
    });
    let second = Arc::new(Notify {
        wakes: AtomicUsize::new(0),
        tx: tx2,
    });
    let w1 = Waker::from(first.clone());
    let w2 = Waker::from(second.clone());
    assert!(
        read.as_mut()
            .poll(&mut Context::from_waker(&w1))
            .is_pending()
    );
    assert!(
        read.as_mut()
            .poll(&mut Context::from_waker(&w1))
            .is_pending()
    );
    assert!(
        read.as_mut()
            .poll(&mut Context::from_waker(&w2))
            .is_pending()
    );
    assert_eq!(
        support::ready(FileStorage.read_dir_async(&fixture.root)).unwrap(),
        vec![std::ffi::OsString::from("fifo")]
    );
    assert_eq!(
        support::ready(FileStorage.modified_async(&fifo)).unwrap(),
        FileStorage.modified(&fifo).unwrap()
    );
    assert!(
        read.as_mut()
            .poll(&mut Context::from_waker(&w2))
            .is_pending()
    );
    release.release();
    rx2.recv().unwrap();
    assert_eq!(second.wakes.load(Ordering::SeqCst), 1);
    assert_eq!(first.wakes.load(Ordering::SeqCst), 0);
    assert!(rx1.try_recv().is_err());
    assert_eq!(support::ready(read).unwrap(), b"released");

    let mut abandoned = FileStorage.read_file_async(&fifo);
    let mut release = Release(Some(
        std::fs::OpenOptions::new().write(true).open(&fifo).unwrap(),
    ));
    assert!(
        abandoned
            .as_mut()
            .poll(&mut Context::from_waker(&w2))
            .is_pending()
    );
    drop(abandoned);
    release.release();
    rx2.recv().unwrap();
    assert_eq!(second.wakes.load(Ordering::SeqCst), 2);
    assert!(support::ready(FileStorage.read_dir_async(&fixture.root)).is_ok());
}

#[test]
fn native_directory_order_matches_filename_bytes() {
    use maestro_storage::{Dirent, DirentKind, Storage};
    let root = std::path::Path::new("controlled-order");
    let controlled = support::Controlled::new(root);
    let names = ["z", "a", "B", "b", "A", "é"]
        .map(std::ffi::OsString::from)
        .to_vec();
    controlled.entries(
        root,
        names
            .iter()
            .map(|name| Dirent {
                name: name.clone(),
                kind: DirentKind::File,
            })
            .collect(),
    );
    assert_eq!(controlled.read_dir(root).unwrap(), names);
    assert_eq!(
        support::ready(controlled.read_dir_async(root)).unwrap(),
        names
    );
    assert_eq!(
        support::ready(controlled.read_dir_with_file_types_async(root))
            .unwrap()
            .into_iter()
            .map(|entry| entry.name)
            .collect::<Vec<_>>(),
        names
    );
    #[cfg(unix)]
    {
        use maestro_storage::FileStorage;
        let fixture = support::Fixture::new("order", false);
        for name in ["z", "a", "B", "b", "A", "é"] {
            FileStorage
                .write_file(&fixture.root.join(name), b"")
                .unwrap();
        }
        let expected = ["A", "B", "a", "b", "z", "é"].map(std::ffi::OsString::from);
        assert_eq!(FileStorage.read_dir(&fixture.root).unwrap(), expected);
        assert_eq!(
            support::ready(FileStorage.read_dir_async(&fixture.root)).unwrap(),
            expected
        );
        assert_eq!(
            support::ready(FileStorage.read_dir_with_file_types_async(&fixture.root))
                .unwrap()
                .into_iter()
                .map(|entry| entry.name)
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn transcript_documentation_matches_byte_examples() {
    assert_eq!(
        include_str!("../../../docs/transcript.md")
            .split("\n\n")
            .collect::<Vec<_>>(),
        support::GUIDE.split("\n\n").collect::<Vec<_>>()
    );
    assert_eq!(
        include_str!("../../../docs/storage.md")
            .split("\n\n")
            .collect::<Vec<_>>(),
        support::POINTER.split("\n\n").collect::<Vec<_>>()
    );
    #[cfg(not(target_arch = "wasm32"))]
    {
        use maestro_storage::{FileStorage, Storage};
        let fixture = support::Fixture::new("documentation", false);
        let directory = fixture.root.join("nested");
        FileStorage.mkdir(&directory).unwrap();
        let file = directory.join("bytes");
        FileStorage.write_file(&file, b"\0\xff").unwrap();
        FileStorage.append_file(&file, b"\r\n").unwrap();
        assert_eq!(FileStorage.read_file(&file).unwrap(), b"\0\xff\r\n");
    }
}
