//! Package records preserve the host wire format at the author boundary.
use maestro_package::{GitPinStatus, ResolvedPackage};
use serde_json::json;

#[test]
fn pin_observations_preserve_the_wire_cleanliness_decision() {
    for (expected, head, paths, clean) in [
        (Some("pin"), Some("pin"), vec![], true),
        (Some("pin"), Some("head"), vec![], false),
        (Some("pin"), Some("pin"), vec!["dirty"], false),
        (None, None, vec![], false),
    ] {
        let wire = json!({
            "expectedCommit": expected,
            "headCommit": head,
            "changedPaths": paths,
            "clean": clean,
        });
        let status: GitPinStatus = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(status).unwrap(), wire);
    }
}

#[test]
fn resolved_packages_decode_fields_and_reject_malformed_results() {
    let wire = json!({
        "source": "./empty", "identity": "local:/agent/empty",
        "scope": "user", "installedPath": "/agent/empty",
    });
    let package: ResolvedPackage<String> = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(package.installed_path, "/agent/empty");
    assert_eq!(package.scope, "user");
    assert_eq!(serde_json::to_value(package).unwrap(), wire);
    for malformed in [json!(null), json!({}), json!({"source": 17})] {
        assert!(serde_json::from_value::<ResolvedPackage<String>>(malformed).is_err());
    }
}
