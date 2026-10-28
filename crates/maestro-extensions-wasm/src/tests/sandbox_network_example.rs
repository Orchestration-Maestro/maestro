//! The example's shared domain-admission seam, without DNS or internet access.

#[path = "../../../../examples/extensions/sandbox/network.rs"]
pub(super) mod network;

#[test]
fn sandbox_domains_preserve_defaults_and_apply_denials_before_wildcards() {
    let defaults = network::NetworkConfig::default();
    assert_eq!(
        serde_json::to_value(&defaults).unwrap()["allowedDomains"],
        serde_json::json!([
            "npmjs.org",
            "*.npmjs.org",
            "registry.npmjs.org",
            "registry.yarnpkg.com",
            "pypi.org",
            "*.pypi.org",
            "github.com",
            "*.github.com",
            "api.github.com",
            "raw.githubusercontent.com"
        ])
    );
    for domain in [
        "npmjs.org",
        "registry.yarnpkg.com",
        "pypi.org",
        "github.com",
        "API.GITHUB.COM",
        "raw.githubusercontent.com",
    ] {
        assert!(defaults.allows(domain), "default denied {domain}");
    }
    assert!(!defaults.allows("github.com.evil.test"));
    let rules = network::NetworkConfig {
        allowed_domains: vec!["*.example.test".into(), "exact.test".into()],
        denied_domains: vec![
            "blocked.example.test".into(),
            "*.private.example.test".into(),
        ],
        ..network::NetworkConfig::default()
    };
    for (domain, allowed) in [
        ("EXACT.TEST", true),
        ("child.exact.test", false),
        ("example.test", false),
        ("child.example.test", true),
        ("deep.child.example.test", true),
        ("BLOCKED.EXAMPLE.TEST", false),
        ("child.private.example.test", false),
        ("private.example.test", true),
        ("notexample.test", false),
        ("", false),
    ] {
        assert_eq!(rules.allows(domain), allowed, "domain {domain}");
    }
}
