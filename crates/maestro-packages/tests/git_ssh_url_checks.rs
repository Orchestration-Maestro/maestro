//! Repository source behavior through the public parser.
use maestro_packages::{GitSource, parse_git_url};
use serde::Deserialize;

/// A consumed repository expectation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Clone address.
    repo: String,
    /// Host identity.
    host: String,
    /// Path identity.
    path: String,
    /// Selected ref.
    #[serde(rename = "ref")]
    r#ref: Option<String>,
    /// Pin flag.
    pinned: bool,
}
/// One unique parser observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Vector {
    /// Owning behavior.
    test: String,
    /// Source spelling.
    input: String,
    /// Selected result.
    expected: Option<Expected>,
}
/// Exercises every row owned by this behavior, rejecting unread fields.
fn vectors(test: &str) -> serde_json::Result<()> {
    let rows: Vec<Vector> = serde_json::from_str(include_str!("git_vectors.json"))?;
    let mut unique = std::collections::HashSet::new();
    for row in &rows {
        assert!(unique.insert(row.input.as_str()), "duplicate parser query");
        assert!(
            matches!(
                row.test.as_str(),
                "https_transport_has_host_path_and_clone_url"
                    | "ssh_transport_keeps_user_and_clone_url"
                    | "protocol_ref_is_separate_from_clone_url"
                    | "prefixed_scp_keeps_ssh_clone_url"
                    | "prefixed_host_path_uses_https"
                    | "prefixed_scp_ref_pins_source"
                    | "bare_scp_is_not_a_git_url"
                    | "bare_host_path_is_not_a_git_url"
                    | "bare_owner_repo_is_not_a_git_url"
                    | "git_whitespace_and_case_follow_admission_branches"
                    | "git_clone_targets_follow_the_selected_repository"
                    | "git_hosted_identities_follow_provider_paths"
                    | "git_generic_admission_keeps_segment_boundaries"
                    | "git_refs_credentials_ports_and_suffixes_stay_distinct"
                    | "git_url_escaping_uses_hosted_or_generic_identity"
                    | "git_hosted_protocols_choose_provider_or_generic_rules"
            ),
            "unvisited row owner: {}",
            row.test
        );
    }
    let mut visited = 0;
    for row in rows.into_iter().filter(|row| row.test == test) {
        visited += 1;
        let expected = row.expected.map(|v| GitSource {
            repo: v.repo,
            host: v.host,
            path: v.path,
            r#ref: v.r#ref,
            pinned: v.pinned,
        });
        assert_eq!(parse_git_url(&row.input), expected, "{}", row.input);
    }
    assert!(visited > 0, "no vectors for {test}");
    Ok(())
}

#[test]
fn https_transport_has_host_path_and_clone_url() {
    vectors("https_transport_has_host_path_and_clone_url").unwrap();
}
#[test]
fn ssh_transport_keeps_user_and_clone_url() {
    vectors("ssh_transport_keeps_user_and_clone_url").unwrap();
}
#[test]
fn protocol_ref_is_separate_from_clone_url() {
    vectors("protocol_ref_is_separate_from_clone_url").unwrap();
}
#[test]
fn prefixed_scp_keeps_ssh_clone_url() {
    vectors("prefixed_scp_keeps_ssh_clone_url").unwrap();
}
#[test]
fn prefixed_host_path_uses_https() {
    vectors("prefixed_host_path_uses_https").unwrap();
}
#[test]
fn prefixed_scp_ref_pins_source() {
    vectors("prefixed_scp_ref_pins_source").unwrap();
}
#[test]
fn bare_scp_is_not_a_git_url() {
    vectors("bare_scp_is_not_a_git_url").unwrap();
}
#[test]
fn bare_host_path_is_not_a_git_url() {
    vectors("bare_host_path_is_not_a_git_url").unwrap();
}
#[test]
fn bare_owner_repo_is_not_a_git_url() {
    vectors("bare_owner_repo_is_not_a_git_url").unwrap();
}
#[test]
fn git_whitespace_and_case_follow_admission_branches() {
    vectors("git_whitespace_and_case_follow_admission_branches").unwrap();
}
#[test]
fn git_clone_targets_follow_the_selected_repository() {
    vectors("git_clone_targets_follow_the_selected_repository").unwrap();
}
#[test]
fn git_hosted_identities_follow_provider_paths() {
    vectors("git_hosted_identities_follow_provider_paths").unwrap();
}
#[test]
fn git_generic_admission_keeps_segment_boundaries() {
    vectors("git_generic_admission_keeps_segment_boundaries").unwrap();
}
#[test]
fn git_refs_credentials_ports_and_suffixes_stay_distinct() {
    vectors("git_refs_credentials_ports_and_suffixes_stay_distinct").unwrap();
}
#[test]
fn git_url_escaping_uses_hosted_or_generic_identity() {
    vectors("git_url_escaping_uses_hosted_or_generic_identity").unwrap();
}
#[test]
fn git_hosted_protocols_choose_provider_or_generic_rules() {
    vectors("git_hosted_protocols_choose_provider_or_generic_rules").unwrap();
}
