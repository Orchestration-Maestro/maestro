//! Keeps native fixture setup selection equal to the declared test-process callers.

use std::{collections::BTreeSet, path::Path};

use cargo_metadata::Metadata;
use toml::Value;

use crate::graph::{
    metadata::members,
    policy::{TEST_PROCESS, TEST_PROCESS_CALLERS},
};

/// Require the fixture owner's workspace to select every declared caller, and no other crate.
pub(crate) fn check(root: &Path, metadata: &Metadata) -> Result<(), String> {
    if !members(metadata).any(|package| package.name.as_str() == TEST_PROCESS) {
        return Ok(());
    }
    let path = root.join(".config/nextest.toml");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let config: Value =
        toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    let selected = selected(&config)?;
    let expected: BTreeSet<_> = TEST_PROCESS_CALLERS
        .iter()
        .copied()
        .chain([TEST_PROCESS])
        .collect();
    let diagnostics: Vec<_> = expected
        .difference(&selected)
        .map(|name| format!("missing package(={name})"))
        .chain(
            selected
                .difference(&expected)
                .map(|name| format!("extra package(={name})")),
        )
        .collect();
    if diagnostics.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{}: native-fixture selection must match TEST_PROCESS_CALLERS plus {TEST_PROCESS}:\n{}",
        path.display(),
        diagnostics.join("\n")
    ))
}

/// Read package atoms from default-profile native-fixture rules with a declared setup.
fn selected(config: &Value) -> Result<BTreeSet<&str>, String> {
    let mut packages = BTreeSet::new();
    if config
        .get("scripts")
        .and_then(|scripts| scripts.get("setup"))
        .and_then(|setups| setups.get("native-fixture"))
        .is_none()
    {
        return Ok(packages);
    }
    let rules = config
        .get("profile")
        .and_then(|profiles| profiles.get("default"))
        .and_then(|profile| profile.get("scripts"))
        .and_then(Value::as_array);
    for rule in rules
        .into_iter()
        .flatten()
        .filter(|rule| rule.get("setup").and_then(Value::as_str) == Some("native-fixture"))
    {
        let filter = rule
            .get("filter")
            .and_then(Value::as_str)
            .ok_or("native-fixture rule requires a filter string in .config/nextest.toml")?;
        for term in filter.split('|').map(str::trim) {
            let name = term
                .strip_prefix("package(=")
                .and_then(|name| name.strip_suffix(')'))
                .filter(|name| {
                    !name.is_empty()
                        && name
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
                })
                .ok_or_else(|| {
                    format!(
                        ".config/nextest.toml: native-fixture filter terms must be exactly package(=NAME), found {term:?}"
                    )
                })?;
            packages.insert(name);
        }
    }
    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargo_metadata::{Metadata, MetadataCommand};
    use maestro_test_directory::Scratch;

    /// A complete native-fixture rule built from the declared caller policy.
    fn config() -> String {
        let filter = TEST_PROCESS_CALLERS
            .iter()
            .copied()
            .chain([TEST_PROCESS])
            .map(|name| format!("package(={name})"))
            .collect::<Vec<_>>()
            .join("\n| ");
        format!(
            "[scripts.setup.native-fixture]\ncommand = 'development nextest-setup native-fixture'\n[[profile.default.scripts]]\nsetup = 'native-fixture'\nfilter = \"\"\"\n{filter}\n\"\"\"\n"
        )
    }

    /// A workspace containing the fixture owner and the supplied nextest configuration.
    fn workspace(config: &str) -> (Scratch, Metadata) {
        let root = Scratch::new().unwrap();
        std::fs::create_dir(root.path().join("src")).unwrap();
        std::fs::write(root.path().join("src/lib.rs"), "").unwrap();
        std::fs::write(
            root.path().join("Cargo.toml"),
            "[package]\nname = 'maestro-test-process'\nversion = '0.1.0'\n[workspace]\n",
        )
        .unwrap();
        std::fs::create_dir(root.path().join(".config")).unwrap();
        std::fs::write(root.path().join(".config/nextest.toml"), config).unwrap();
        let metadata = MetadataCommand::new()
            .current_dir(root.path())
            .no_deps()
            .other_options(vec!["--offline".into()])
            .exec()
            .unwrap();
        (root, metadata)
    }

    #[test]
    fn native_fixture_setup_serves_every_test_process_caller() {
        let config = config().replace("| package(=maestro-cli)\n", "");
        let (root, metadata) = workspace(&config);

        let error = check(root.path(), &metadata).unwrap_err();

        assert!(error.contains("missing package(=maestro-cli)"), "{error}");
    }

    #[test]
    fn native_fixture_setup_names_only_test_process_callers() {
        let config = config().replace(
            "package(=maestro-cli)",
            "package(=maestro-cli) | package(=maestro-text) | package(=maestro-json)",
        );
        let (root, metadata) = workspace(&config);

        let error = check(root.path(), &metadata).unwrap_err();

        for name in ["maestro-text", "maestro-json"] {
            assert!(
                error.contains(&format!("extra package(={name})")),
                "{error}"
            );
        }
    }

    #[test]
    fn matching_native_fixture_selection_passes() {
        let (root, metadata) = workspace(&config());

        assert_eq!(check(root.path(), &metadata), Ok(()));
    }

    #[test]
    fn other_profiles_cannot_supply_missing_default_fixture_callers() {
        let config = format!(
            "{}\n[[profile.ci.scripts]]\nsetup = 'native-fixture'\nfilter = 'package(=maestro-cli)'\n",
            config().replace("| package(=maestro-cli)\n", "")
        );
        let (root, metadata) = workspace(&config);

        let error = check(root.path(), &metadata).unwrap_err();

        assert!(error.contains("missing package(=maestro-cli)"), "{error}");
    }

    #[test]
    fn native_fixture_selection_rejects_narrowed_package_terms() {
        let config = config().replace(
            "package(=maestro-cli)",
            "package(=maestro-cli) & test(startup_stdout::)",
        );
        let (root, metadata) = workspace(&config);

        let error = check(root.path(), &metadata).unwrap_err();

        assert!(error.contains("package(=NAME)"), "{error}");
        assert!(error.contains("startup_stdout::"), "{error}");
    }

    #[test]
    fn native_fixture_selection_rejects_negated_package_terms() {
        let config = config().replace("package(=maestro-cli)", "not package(=maestro-cli)");
        let (root, metadata) = workspace(&config);

        let error = check(root.path(), &metadata).unwrap_err();

        assert!(error.contains("package(=NAME)"), "{error}");
        assert!(error.contains("not package(=maestro-cli)"), "{error}");
    }

    #[test]
    fn fixture_owner_requires_native_fixture_setup() {
        let (root, metadata) = workspace("");

        let error = check(root.path(), &metadata).unwrap_err();

        for name in ["maestro-test-process", "maestro-cli", "maestro-agent"] {
            assert!(
                error.contains(&format!("missing package(={name})")),
                "{error}"
            );
        }
    }
}
