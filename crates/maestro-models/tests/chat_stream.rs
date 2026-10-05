mod support;
use maestro_models::*;
use support::conformance::*;

#[test]
fn tool_metadata_rejects_nonopen_or_wrong_family_blocks() {
    for prefix in [
        vec![],
        text(0, "hello"),
        vec![
            tool_start(0),
            ProviderUpdate::ToolCallDelta {
                content_index: 0,
                delta: "{}".into(),
            },
            ProviderUpdate::ToolCallEnd { content_index: 0 },
        ],
    ] {
        let mut updates = prefix;
        updates.push(ProviderUpdate::ToolCallMetadata {
            content_index: 0,
            id: Some("late".into()),
            name: None,
            replay_metadata: None,
        });
        updates.push(done());
        assert_eq!(
            terminal(&run(updates)).failure,
            Some(Failure::MalformedStream)
        );
    }
}
