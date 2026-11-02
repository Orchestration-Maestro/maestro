//! The packages that consume the prepared test setup, as declared for shared CI.

/// The preparation job runs for exactly the packages that nextest gives the author-component
/// or browser-author setup script, so a run that selects none of them skips the job.
#[test]
fn ci_declares_the_packages_that_read_the_prepared_setup() {
    let nextest: toml::Table = include_str!("../../../.config/nextest.toml")
        .parse()
        .unwrap();
    let rules = nextest["profile"]["default"]["scripts"].as_array().unwrap();
    let mut consumers: Vec<&str> = rules
        .iter()
        .filter(|rule| {
            matches!(
                rule["setup"].as_str(),
                Some("author-component" | "browser-author")
            )
        })
        .flat_map(|rule| rule["filter"].as_str().unwrap().split("package(").skip(1))
        .filter_map(|rest| rest.split(')').next())
        .map(|name| name.trim_start_matches('='))
        .collect();
    consumers.sort_unstable();
    consumers.dedup();
    let ci: toml::Table = include_str!("../../../.github/ci.toml").parse().unwrap();
    let mut declared: Vec<&str> = ci["test_prepare_packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap())
        .collect();
    declared.sort_unstable();
    assert_eq!(declared, consumers);
}
