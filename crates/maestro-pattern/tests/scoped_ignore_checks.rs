//! Public directory-scope decisions.
use maestro_pattern::ignore::{Ignore, ScopedIgnoreRules, TestResult};

/// Deeper decisions override ancestors without parsing directory identities.
#[test]
fn scoped_rules_keep_deepest_decisions_and_literal_roots() {
    let mut root = Ignore::default();
    assert_eq!(Ignore::new().test("drop.md", false), TestResult::Unmatched);
    assert_eq!(root.test("drop.md", false), TestResult::Unmatched);
    root.add("*.md");
    assert_eq!(root.test("drop.md", false), TestResult::Ignored);
    root.add("!/keep.md");
    assert_eq!(root.test("keep.md", false), TestResult::Unignored);
    let mut scopes = ScopedIgnoreRules::default();
    assert!(!scopes.ignored("drop.md", false));
    scopes.push(String::new(), root);
    let mut local = Ignore::new();
    local.add("!keep.md");
    scopes.push("[ab]".into(), local);
    assert!(!scopes.ignored("[ab]/keep.md", false));
    assert!(scopes.ignored("[ab]/drop.md", false));
    assert!(scopes.ignored("a/keep.md", false));
    let mut local = Ignore::new();
    local.add("!boundary.md");
    scopes.push("a".into(), local);
    assert!(!scopes.ignored("a/boundary.md", false));
    assert!(scopes.ignored("ab/boundary.md", false));
    assert!(!scopes.ignored("[ab]/plain.txt", false));
}

/// Removing a scope changes the same candidate back to its parent's decision.
#[test]
fn scoped_rules_pop_restores_parent_decision() {
    let mut scopes = ScopedIgnoreRules::default();
    let mut root = Ignore::new();
    root.add("*.md");
    scopes.push(String::new(), root);
    let mut local = Ignore::new();
    local.add("!keep.md");
    scopes.push("nested".into(), local);
    assert!(!scopes.ignored("nested/keep.md", false));
    scopes.pop();
    assert!(scopes.ignored("nested/keep.md", false));
    scopes.pop();
    assert!(!scopes.ignored("nested/keep.md", false));
    scopes.pop();
    assert!(!scopes.ignored("nested/keep.md", false));
}
