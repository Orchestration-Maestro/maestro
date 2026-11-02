//! Unit coverage for the lock graph.

use super::Lock;

/// A lock file with a workspace crate `w` that builds on `a`; `a` builds on `b 1.0.0` and `c`;
/// `b` builds on `c`; `d` is needed by nothing; `b` is locked at two versions.
const LOCK: &str = "version = 4\n\n\
    [[package]]\nname = \"w\"\nversion = \"0.1.0\"\ndependencies = [\n \"a\",\n]\n\n\
    [[package]]\nname = \"a\"\nversion = \"1.0.0\"\nsource = \"registry+x\"\n\
    dependencies = [\n \"b 1.0.0\",\n \"c\",\n]\n\n\
    [[package]]\nname = \"b\"\nversion = \"1.0.0\"\nsource = \"registry+x\"\n\
    dependencies = [\n \"c\",\n]\n\n\
    [[package]]\nname = \"b\"\nversion = \"2.0.0\"\nsource = \"registry+x\"\n\n\
    [[package]]\nname = \"c\"\nversion = \"1.0.0\"\nsource = \"registry+x\"\n\n\
    [[package]]\nname = \"d\"\nversion = \"1.0.0\"\nsource = \"registry+x\"\n";

/// The names and versions the closure of `roots` holds over the packages `keep` accepts.
fn reached(roots: &[&str], keep: impl Fn(&super::LockedPackage) -> bool) -> Vec<String> {
    let lock: Lock = toml::from_str(LOCK).unwrap();
    lock.graph(keep)
        .closure(roots.iter().copied())
        .iter()
        .map(|package| format!("{} {}", package.name, package.version))
        .collect()
}

/// The closure holds the named packages and everything they build on, each version of a name an
/// entry gives without one, and leaves out the packages that nothing named needs.
#[test]
fn closure_holds_the_named_packages_and_what_they_build_on() {
    assert_eq!(
        reached(&["a"], |_| true),
        ["a 1.0.0", "b 1.0.0", "b 2.0.0", "c 1.0.0"]
    );
    assert_eq!(reached(&["b"], |_| true), ["b 1.0.0", "b 2.0.0", "c 1.0.0"]);
    assert!(reached(&["missing"], |_| true).is_empty());
}

/// A package the filter rejects is neither reached nor walked through.
#[test]
fn closure_follows_only_the_accepted_packages() {
    assert_eq!(
        reached(&["w"], |package| package.source.is_none()),
        ["w 0.1.0"]
    );
}
