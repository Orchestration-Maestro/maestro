//! Enabled package contribution files read for the browser frontend.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use maestro_app::{
    PathMetadata, ResolvedResource, SourceOrigin, SourceScope, contributions::load_contributions,
};
use maestro_resources::DiagnosticType;
use std::{fs, path::Path};

/// A resolved contribution file from `source`.
fn resource(path: &Path, source: &str, enabled: bool) -> ResolvedResource {
    ResolvedResource {
        path: path.to_string_lossy().into_owned(),
        enabled,
        metadata: PathMetadata {
            source: source.into(),
            scope: SourceScope::User,
            origin: SourceOrigin::Package,
            base_dir: None,
        },
    }
}
/// Write one file exclusively.
fn write(path: &Path, text: &str) {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, text.as_bytes()))
        .unwrap();
}

/// Enabled files load in resolution order with their supplying package; disabled ones are skipped.
#[test]
fn contributions_are_read_from_enabled_packages_in_order() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();
    for (name, id) in [
        ("b.json", "second"),
        ("a.json", "first"),
        ("off.json", "off"),
    ] {
        write(
            &root.join(name),
            &format!(r#"{{"id":"{id}","kind":"card","target":"home"}}"#),
        );
    }
    let (accepted, diagnostics) = load_contributions(&[
        resource(&root.join("b.json"), "npm:one", true),
        resource(&root.join("off.json"), "npm:one", false),
        resource(&root.join("a.json"), "npm:two", true),
    ]);
    assert!(diagnostics.is_empty());
    let loaded: Vec<(&str, &str)> = accepted
        .iter()
        .map(|c| (c.package.as_str(), c.descriptor.id.as_str()))
        .collect();
    assert_eq!(loaded, [("npm:one", "second"), ("npm:two", "first")]);
}

/// An unreadable or invalid file yields one warning naming it, and the rest still load.
#[test]
fn invalid_contribution_files_report_a_warning_and_are_skipped() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();
    write(&root.join("bad.json"), r#"{"id":"bad","kind":"card"}"#);
    write(
        &root.join("route.json"),
        r#"{"id":"r","kind":"page","target":"pages","route":3}"#,
    );
    write(
        &root.join("good.json"),
        r#"{"id":"good","kind":"card","target":"home"}"#,
    );
    let missing = root.join("missing.json");
    let (accepted, diagnostics) = load_contributions(&[
        resource(&root.join("bad.json"), "pkg", true),
        resource(&missing, "pkg", true),
        resource(&root.join("route.json"), "pkg", true),
        resource(&root.join("good.json"), "pkg", true),
    ]);
    assert_eq!(accepted.len(), 1);
    assert_eq!(accepted[0].descriptor.id, "good");
    let reported: Vec<_> = diagnostics
        .iter()
        .map(|d| (d.r#type == DiagnosticType::Warning, d.path.clone()))
        .collect();
    let path = |name: &str| Some(root.join(name).to_string_lossy().into_owned());
    assert_eq!(
        reported,
        [
            (true, path("bad.json")),
            (true, path("missing.json")),
            (true, path("route.json"))
        ]
    );
    assert!(
        diagnostics
            .iter()
            .all(|d| !d.message.is_empty() && d.collision.is_none())
    );
}
