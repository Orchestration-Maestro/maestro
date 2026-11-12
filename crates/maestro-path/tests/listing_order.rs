//! Directory listing order on the native file system.
#![cfg(not(target_arch = "wasm32"))]

use std::{fs, path::Path};

#[test]
fn entries_come_sorted_by_name_whatever_the_file_system_order() {
    let root = &Path::new(env!("CARGO_TARGET_TMPDIR")).join("listing-order");
    // An interrupted run leaves its directory behind.
    let _ = fs::remove_dir_all(root);
    fs::create_dir_all(root).unwrap();

    // NTFS returns these case-insensitively (a, b, C, _x); name order is byte order.
    for name in ["b", "_x", "a"] {
        fs::write(root.join(name), name).unwrap();
    }
    fs::create_dir(root.join("C")).unwrap();

    let names = maestro_path::fs::read_dir(root).map(|entries| {
        entries
            .iter()
            .map(fs::DirEntry::file_name)
            .collect::<Vec<_>>()
    });
    fs::remove_dir_all(root).unwrap();

    assert_eq!(names.unwrap(), ["C", "_x", "a", "b"]);
}
