//! Platform viewer scan cost on production sources.

use super::{IMPORT_SCOPES, POSITION_LOOKUPS, check};
use crate::source::Member;

#[test]
fn production_scans_skip_positions_and_reuse_import_scopes() {
    let member = Member::with_sources(
        "maestro-models",
        "models",
        [(
            "models/src/lib.rs",
            "use std::process::Command;\nfn view() { run((1), [2], { Command::new(\"viewer\") }); }\n",
        )],
    );
    POSITION_LOOKUPS.set(0);
    IMPORT_SCOPES.set(0);
    assert_eq!(check(&[member]), Ok(()));
    let work = (POSITION_LOOKUPS.get(), IMPORT_SCOPES.get());
    println!("position lookups, import scopes: {work:?}");
    // No test item needs token positions, and only the file scope declares imports.
    assert_eq!(work, (0, 1));
}
