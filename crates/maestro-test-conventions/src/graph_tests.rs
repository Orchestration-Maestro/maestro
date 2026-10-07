use super::graph::*;
use serde_json::{Value, json};

fn metadata(from: &str, to: &str, kind: &Value) -> Value {
    json!({"workspace_members": ["source-id", "target-id"], "packages": [
        {"id": "source-id", "name": from, "dependencies": []},
        {"id": "target-id", "name": to, "dependencies": []}
    ], "resolve": {"nodes": [
        {"id": "source-id", "deps": [{"name": "harmless_alias", "pkg": "target-id", "dep_kinds": [{"kind": kind, "target": null}]}]},
        {"id": "target-id", "deps": []}
    ]}})
}

#[test]
fn resolved_edges_are_validated_by_package_identity_and_kind() {
    for kind in [Value::Null, json!("build"), json!("dev")] {
        let value = metadata("maestro-cli", "maestro-storage", &kind);
        let edges = resolved(&value).unwrap();
        let error = validate(&value, &edges).unwrap_err();
        if kind == "dev" {
            assert!(error.contains("test support"), "{error}");
        } else {
            assert!(error.contains("forbidden production dependency"), "{error}");
        }
    }
    let allowed = metadata("maestro-session", "maestro-storage", &Value::Null);
    assert_eq!(validate(&allowed, &resolved(&allowed).unwrap()), Ok(()));
    let mut external = allowed.clone();
    external["workspace_members"] = json!(["source-id"]);
    external["packages"][1]["name"] = json!("external-storage");
    assert!(resolved(&external).unwrap().is_empty());
}

#[test]
fn missing_or_malformed_resolve_is_rejected() {
    let valid = metadata("maestro-session", "maestro-storage", &Value::Null);
    let mut malformed = Vec::new();
    let mut value = valid.clone();
    value.as_object_mut().unwrap().remove("resolve");
    malformed.push(value);
    let mut value = valid.clone();
    value["resolve"] = Value::Null;
    malformed.push(value);
    let mut value = valid.clone();
    value["resolve"]["nodes"] = json!([]);
    malformed.push(value);
    let mut value = valid.clone();
    value["resolve"]["nodes"][0]["deps"][0]["dep_kinds"] = json!([]);
    malformed.push(value);
    let mut value = valid.clone();
    value["resolve"]["nodes"][0]["deps"][0]
        .as_object_mut()
        .unwrap()
        .remove("dep_kinds");
    malformed.push(value);
    let mut value = valid.clone();
    value["resolve"]["nodes"][0]["deps"][0]["dep_kinds"][0]
        .as_object_mut()
        .unwrap()
        .remove("kind");
    malformed.push(value);
    let mut value = valid.clone();
    value["resolve"]["nodes"][0]["deps"][0]["pkg"] = json!("unknown");
    malformed.push(value);
    let mut value = valid.clone();
    value["resolve"]["nodes"][0]["deps"][0]["dep_kinds"][0]["kind"] = json!("unknown");
    malformed.push(value);
    let mut value = valid.clone();
    value["resolve"]["nodes"][0]["id"] = json!("unknown");
    malformed.push(value);
    let mut value = valid.clone();
    value["resolve"]["nodes"].as_array_mut().unwrap().pop();
    malformed.push(value);
    let mut value = valid.clone();
    value["resolve"]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(valid["resolve"]["nodes"][0].clone());
    malformed.push(value);
    for value in malformed {
        assert!(
            resolved(&value).is_err(),
            "accepted malformed metadata: {value}"
        );
    }
}

#[test]
fn internal_dev_edges_require_declared_dependency_free_support() {
    let value = metadata("maestro-session", "maestro-test-support", &json!("dev"));
    let edges = resolved(&value).unwrap();
    assert!(validate(&value, &edges).is_err());
    assert_eq!(
        validate_with_support(&value, &edges, &["maestro-test-support"]),
        Ok(())
    );
    assert!(validate_with_support(&value, &edges, &["maestro-test-other"]).is_err());
    let mut dependent = value.clone();
    dependent["packages"][1]["dependencies"] = json!([{ "name": "maestro-session", "kind": null }]);
    assert!(validate_with_support(&dependent, &edges, &["maestro-test-support"]).is_err());
    let normal = metadata("maestro-session", "maestro-test-support", &Value::Null);
    assert!(
        validate_with_support(
            &normal,
            &resolved(&normal).unwrap(),
            &["maestro-test-support"]
        )
        .is_err()
    );
    let dedicated = metadata("maestro-session", "maestro", &json!("dev"));
    assert!(
        validate_with_support(&dedicated, &resolved(&dedicated).unwrap(), &["maestro"]).is_err()
    );
    let leaf = metadata("maestro-storage", "maestro-test-support", &json!("dev"));
    assert!(
        validate_with_support(&leaf, &resolved(&leaf).unwrap(), &["maestro-test-support"]).is_err()
    );
}

#[test]
fn test_graph_cycles_are_rejected_separately() {
    let mut value = metadata("maestro-session", "maestro-agent", &Value::Null);
    value["resolve"]["nodes"][1]["deps"] = json!([{ "name": "alias", "pkg": "source-id", "dep_kinds": [{ "kind": "dev", "target": null }] }]);
    let error = validate(&value, &resolved(&value).unwrap()).unwrap_err();
    assert_eq!(
        error,
        "test dependency cycle: maestro-agent -> maestro-session -> maestro-agent"
    );
    value["resolve"]["nodes"][1]["deps"] = json!([]);
    assert_eq!(validate(&value, &resolved(&value).unwrap()), Ok(()));
    value["resolve"]["nodes"][0]["deps"][0]["dep_kinds"][0]["kind"] = json!("dev");
    assert!(
        validate(&value, &resolved(&value).unwrap())
            .unwrap_err()
            .contains("test support")
    );
}
