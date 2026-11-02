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
    assert!(!ci.contains_key("test_prepare_packages"));
    let setups = nextest["scripts"]["setup"].as_table().unwrap();
    let mut required: Vec<_> = setups
        .iter()
        .filter(|(name, setup)| {
            let command = &setup["command"];
            let command = command
                .as_str()
                .unwrap_or_else(|| command["command-line"].as_str().unwrap());
            command.contains("target/test-setup/development") || *name == "browser-author"
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
                .any(|rule| rule["setup"].as_str() == Some(name)),
            "{name} has no default-profile setup rule"
        );
    }
}
