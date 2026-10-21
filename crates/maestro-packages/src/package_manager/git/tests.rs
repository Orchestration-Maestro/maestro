//! Staging allocation on a real directory.
use super::create_temp_dir;
use std::path::Path;

/// Allocation skips an existing candidate name and keeps its contents.
#[test]
fn staging_allocation_skips_collisions() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let parent = scratch.path().to_str().unwrap();
    let first = create_temp_dir(parent).unwrap();
    let name = Path::new(&first).file_name().unwrap().to_str().unwrap();
    let (prefix, index) = name.rsplit_once('-').unwrap();
    let collision = scratch
        .path()
        .join(format!("{prefix}-{}", index.parse::<usize>().unwrap() + 1));
    std::fs::create_dir(&collision).unwrap();
    std::fs::write(collision.join("foreign"), "retain").unwrap();
    let second = create_temp_dir(parent).unwrap();
    assert_ne!(Path::new(&second), collision);
    assert_eq!(
        std::fs::read_to_string(collision.join("foreign")).unwrap(),
        "retain"
    );
}
