//! The nextest setups whose consumers need the prepared bundle in shared CI.

/// Every bundle-reading setup is declared for preparation and assigned to tests.
#[test]
fn ci_declares_every_setup_that_reads_the_prepared_bundle() {
    let nextest: toml::Table = include_str!("../../../.config/nextest.toml")
        .parse()
        .unwrap();
    let ci: toml::Table = include_str!("../../../.github/ci.toml").parse().unwrap();
    let mut declared: Vec<_> = ci["test_prepare_setups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap())
        .collect();
    assert!(
        !ci.contains_key("test_prepare_packages"),
        "prepared consumers must come from nextest setup filters, not a package list"
    );
    let setups = nextest["scripts"]["setup"].as_table().unwrap();
    let mut required: Vec<_> = setups
        .iter()
        .filter(|(name, setup)| {
            let command = &setup["command"];
            let command = command
                .as_str()
                .unwrap_or_else(|| command["command-line"].as_str().unwrap());
            // These read the bundle but run debug/development instead of the staged tool.
            command.contains("target/test-setup/development")
                || ["browser-author", "workspace-sources"].contains(&name.as_str())
        })
        .map(|(name, _)| name.as_str())
        .collect();
    declared.sort_unstable();
    required.sort_unstable();
    assert_eq!(declared, required);
    let rules = nextest["profile"]["default"]["scripts"].as_array().unwrap();
    for name in declared {
        assert!(
            rules
                .iter()
                .any(|rule| rule.get("setup").and_then(toml::Value::as_str) == Some(name)),
            "{name} has no default-profile setup rule"
        );
    }
}
