//! Reusable public-interface assertions for supplied-path byte adapters.

use crate::Storage;
use std::path::Path;

/// Assert explicit paths, recursive creation and no implicit parent creation.
pub fn explicit_paths_and_recursive_directories(storage: &dyn Storage, root: &Path) {
    let nested = root.join("paths/a/b");
    assert!(!storage.exists(&nested));
    assert!(storage.read_file(&nested).is_err());
    assert!(!storage.exists(&nested));
    assert!(storage.append_file(&nested.join("absent"), b"x").is_err());
    assert!(storage.write_file(&nested.join("absent"), b"x").is_err());
    storage.mkdir(&nested).unwrap();
    storage.mkdir(&nested).unwrap();
    assert!(storage.exists(&nested));
    let file = nested.join("file");
    storage.append_file(&file, b"").unwrap();
    assert!(storage.exists(&file));
    assert_eq!(storage.read_file(&file).unwrap(), b"");
    assert!(storage.mkdir(&file.join("blocked")).is_err());
}

/// Assert raw full reads across text and binary boundaries.
pub fn opaque_file_reads(storage: &dyn Storage, root: &Path) {
    let file = root.join("opaque");
    for bytes in [&b""[..], b"{malformed", b"\0\xff\r\n\xef\xbb\xbf\xc2\x85"] {
        storage.write_file(&file, bytes).unwrap();
        assert_eq!(storage.read_file(&file).unwrap(), bytes);
    }
    let missing = root.join("missing-read");
    assert_eq!(
        storage.read_file(&missing).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    assert!(!storage.exists(&missing));
}

/// Assert one bounded prefix read returns only available bytes.
pub fn prefix_reads_keep_bytes_and_requested_bound(storage: &dyn Storage, root: &Path) {
    let file = root.join("prefix");
    for size in [0, 3, 512, 514] {
        let bytes = vec![0xff; size];
        storage.write_file(&file, &bytes).unwrap();
        for length in [0, 1, 512, 513] {
            assert_eq!(
                storage.read_prefix(&file, length).unwrap(),
                vec![0xff; size.min(length)]
            );
        }
    }
    storage.write_file(&file, b"\xc3\xa9").unwrap();
    assert_eq!(storage.read_prefix(&file, 1).unwrap(), b"\xc3");
    assert!(
        storage
            .read_prefix(&root.join("missing-prefix"), 0)
            .is_err()
    );
}

/// Assert sequential append preserves chunks, including duplicates and empties.
pub fn append_chunks_without_framing(storage: &dyn Storage, root: &Path) {
    let file = root.join("append");
    for chunk in [&b"header\n"[..], b"header\n", b"\0\xff", b"", b"tail"] {
        storage.append_file(&file, chunk).unwrap();
    }
    assert_eq!(
        storage.read_file(&file).unwrap(),
        b"header\nheader\n\0\xfftail"
    );
    assert!(
        storage
            .append_file(&root.join("missing-parent/file"), b"x")
            .is_err()
    );
}

/// Assert replacement creates a file and removes every old suffix byte.
pub fn rewrite_removes_old_suffix(storage: &dyn Storage, root: &Path) {
    let file = root.join("rewrite");
    storage.write_file(&file, b"long original suffix").unwrap();
    storage.write_file(&file, b"\0\xff").unwrap();
    assert_eq!(storage.read_file(&file).unwrap(), b"\0\xff");
    storage.write_file(&file, b"").unwrap();
    assert_eq!(storage.read_file(&file).unwrap(), b"");
}

/// Assert unfiltered names and separately selected modification times.
pub fn directory_names_and_modification_time_are_separate(storage: &dyn Storage, root: &Path) {
    let dir = root.join("names");
    storage.mkdir(&dir).unwrap();
    assert!(storage.read_dir(&dir).unwrap().is_empty());
    storage.mkdir(&dir.join("folder")).unwrap();
    for name in [".hidden", "notes.txt", "invalid.jsonl"] {
        storage.write_file(&dir.join(name), b"not JSON").unwrap();
    }
    let mut names = storage.read_dir(&dir).unwrap();
    names.sort();
    assert_eq!(
        names,
        [".hidden", "folder", "invalid.jsonl", "notes.txt"].map(std::ffi::OsString::from)
    );
    let file = dir.join("notes.txt");
    assert_eq!(
        storage.modified(&file).unwrap(),
        storage.modified(&file).unwrap()
    );
    assert!(storage.modified(&dir.join("absent")).is_err());
    assert!(storage.read_dir(&root.join("absent-dir")).is_err());
}

/// Assert I/O failures do not poison later calls on the same adapter.
pub fn io_failure_leaves_later_operations_available(storage: &dyn Storage, root: &Path) {
    let parent = root.join("blocking-file");
    storage.write_file(&parent, b"block").unwrap();
    let blocked = parent.join("child");
    assert!(storage.mkdir(&blocked).is_err());
    assert!(storage.write_file(&blocked, b"bad").is_err());
    assert!(storage.append_file(&blocked, b"bad").is_err());
    let usable = root.join("usable");
    storage.write_file(&usable, b"ok").unwrap();
    storage.append_file(&usable, b"!").unwrap();
    assert_eq!(storage.read_file(&usable).unwrap(), b"ok!");
    assert_eq!(
        storage
            .read_file(&root.join("missing-error"))
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::NotFound
    );
}

/// Assert one unchanged byte caller accepts any storage adapter.
pub fn same_byte_caller_accepts_each_adapter(storage: &dyn Storage, root: &Path) {
    let dir = root.join("caller");
    let file = dir.join("bytes");
    storage.mkdir(&dir).unwrap();
    storage.write_file(&file, b"one").unwrap();
    storage.append_file(&file, b"two").unwrap();
    assert_eq!(storage.read_file(&file).unwrap(), b"onetwo");
    assert_eq!(storage.read_prefix(&file, 2).unwrap(), b"on");
    assert_eq!(
        storage.read_dir(&dir).unwrap(),
        [std::ffi::OsString::from("bytes")]
    );
    storage.modified(&file).unwrap();
    assert!(storage.exists(&file));
}
