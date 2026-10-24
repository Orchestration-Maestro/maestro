//! Exact parser errors at the version-classification boundary.
use super::*;

#[test]
fn version_errors_keep_exact_messages_and_decoding_phases() {
    for (version, comparison) in [(0, "older"), (999, "newer")] {
        let input = format!(r#"{{"version":{version},"workspaces":[]}}"#);
        assert_eq!(
            parse_snapshot(&input).err().unwrap(),
            format!("snapshot version {version} is {comparison} than supported 1")
        );
        assert_eq!(
            parse_history_snapshot(&input).err().unwrap(),
            format!("history snapshot version {version} is {comparison} than supported 1")
        );
    }
    assert_eq!(
        parse_snapshot(r#"{"version":999,"workspaces":[false]}"#)
            .err()
            .unwrap(),
        "snapshot version 999 is newer than supported 1"
    );
    assert!(
        parse_history_snapshot(r#"{"version":999,"workspaces":[false]}"#)
            .err()
            .unwrap()
            .contains("expected struct WorkspaceHistorySnapshot")
    );
}
