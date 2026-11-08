//! Generate one pinned-browser test per inventory export, so the test runner's own limits
//! bound and name each export.

use std::fmt::Write as _;

/// The export inventory the web crate's browser tests are registered in.
const INVENTORY: &str = "../maestro-web/tests/sandbox_loading_support/exports.json";

/// The inventory's export families, in their authored order.
const FAMILIES: [&str; 4] = ["execution", "loading", "contributions", "canvas"];

/// Write one module per family, holding one test per export, into Cargo's private output
/// directory.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").ok_or("missing manifest directory")?,
    );
    let inventory = manifest.join(INVENTORY);
    println!("cargo::rerun-if-changed={}", inventory.display());
    let inventory: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(inventory)?)?;
    let mut tests = String::new();
    for family in FAMILIES {
        writeln!(tests, "/// The `{family}` exports.\nmod {family} {{")?;
        for name in inventory[family]
            .as_array()
            .ok_or_else(|| format!("inventory family {family} must be an array"))?
        {
            let name = name
                .as_str()
                .ok_or_else(|| format!("{family} export names must be strings"))?;
            writeln!(
                tests,
                "    /// Runs the `{name}` export.\n    #[test]\n    #[ignore = \"runs in the pinned Chromium job\"]\n    fn {name}() {{\n        super::export({family:?}, {name:?});\n    }}"
            )?;
        }
        tests.push_str("}\n");
    }
    let output = std::path::PathBuf::from(
        std::env::var_os("OUT_DIR").ok_or("missing build output directory")?,
    );
    std::fs::write(output.join("exports.rs"), tests)?;
    Ok(())
}
