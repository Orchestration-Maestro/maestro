//! Directory listing order on the native file system.

use std::fs;

#[test]
fn entries_come_sorted_by_name_whatever_the_file_system_order() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();

    // NTFS returns these case-insensitively (a, b, C, _x); name order is byte order.
    for name in ["b", "_x", "a"] {
        fs::write(root.join(name), name).unwrap();
    }
    fs::create_dir(root.join("C")).unwrap();

    let names = maestro_listing::read_dir(root).map(|entries| {
        entries
            .iter()
            .map(fs::DirEntry::file_name)
            .collect::<Vec<_>>()
    });
    fs::remove_dir_all(root).unwrap();

    assert_eq!(names.unwrap(), ["C", "_x", "a", "b"]);
}
