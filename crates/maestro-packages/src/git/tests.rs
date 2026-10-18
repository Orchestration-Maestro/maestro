//! Checkout admission under the consuming path flavor.

use super::parse_with_separator;

#[test]
fn checkout_admission_uses_the_consuming_separator_rule() {
    let unsafe_sources = [
        r"git:example.com/a\..\..\..\sibling/repo",
        r"git:..\../owner/repo",
        "https://github.com/..%5C..%5Csibling/repo",
    ];
    let admitted: Vec<_> = unsafe_sources
        .iter()
        .map(|source| parse_with_separator(source, maestro_path::win32::is_separator).is_some())
        .collect();
    assert_eq!(admitted, [false; 3], "Windows traversal admission");
    for source in unsafe_sources {
        assert!(
            parse_with_separator(source, maestro_path::posix::is_separator).is_some(),
            "POSIX rejected {source:?}"
        );
    }
    for source in [
        "git:example.com/group/sub/repo",
        "git:example.com/owner/repo/",
        "git:example.com/owner//repo",
        "https://github.com/..hidden/repo",
        r"git:example.com/owner\name/repo",
    ] {
        let posix = parse_with_separator(source, maestro_path::posix::is_separator);
        assert!(posix.is_some(), "safe control {source:?}");
        assert_eq!(
            parse_with_separator(source, maestro_path::win32::is_separator),
            posix,
            "safe identity changed for {source:?}"
        );
    }
}
