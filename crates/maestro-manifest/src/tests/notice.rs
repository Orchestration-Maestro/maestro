//! The notices a load leaves for the user.
use crate::notice::{drifted, ignored, restored};

const SOURCE: &str = "git:github.com/acme/manifests@v1";

fn files(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn a_restore_lists_the_files_it_reset_or_removed() {
    assert_eq!(
        restored(SOURCE, &files(&["core/SYSTEM.md", "notes.txt"])),
        "Manifest release restored: git:github.com/acme/manifests@v1\n  core/SYSTEM.md\n  notes.txt"
    );
    assert_eq!(
        restored(SOURCE, &[]),
        "Manifest release restored: git:github.com/acme/manifests@v1"
    );
}

#[test]
fn unrepairable_drift_names_the_files_or_the_moved_checkout() {
    assert_eq!(
        drifted(SOURCE, &files(&["a", "b"]), "offline"),
        "Manifest release git:github.com/acme/manifests@v1 has drifted (changed: a, b) and could \
         not be restored: offline. Running on the last accepted composition; the next start repairs it."
    );
    assert_eq!(
        drifted(SOURCE, &[], "offline"),
        "Manifest release git:github.com/acme/manifests@v1 has drifted (the checkout is not at \
         its pinned commit) and could not be restored: offline. Running on the last accepted \
         composition; the next start repairs it."
    );
}

#[test]
fn a_set_aside_project_selection_names_both_manifests() {
    assert_eq!(
        ignored("git:github.com/other/m@v1", SOURCE),
        "The project's manifest git:github.com/other/m@v1 is ignored: the user's manifest \
         git:github.com/acme/manifests@v1 locks \"manifest\"."
    );
}
