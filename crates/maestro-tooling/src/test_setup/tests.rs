//! Unit coverage for the environment filter of the preparation build.

use std::ffi::OsString;

use super::inherited_components;

/// Every name component discovery accepts is removed, including the bare and doubled
/// underscore forms, and no other name is.
#[test]
fn inherited_components_follow_the_discovery_rule() {
    let names = [
        "MAESTRO_AUTHOR_COMPONENT",
        "MAESTRO_COMPONENT",
        "MAESTRO__COMPONENT",
        "MAESTRO_HOME",
        "MAESTRO_lower_COMPONENT",
        "PATH",
    ]
    .map(OsString::from);
    assert_eq!(
        inherited_components(names.into_iter()),
        [
            "MAESTRO_AUTHOR_COMPONENT",
            "MAESTRO_COMPONENT",
            "MAESTRO__COMPONENT"
        ]
        .map(OsString::from)
    );
}
