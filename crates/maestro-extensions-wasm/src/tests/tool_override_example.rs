//! Native execution of the same shipped callbacks, with host-supplied helper replies.
use super::example_support::{release_all, result_value};
use crate::component_adapter::ToolCapabilities;
use crate::{Extension, ExtensionAPI, ExtensionFuture};
use serde_json::json;

#[path = "../../../../examples/extensions/tool_override.rs"]
mod tool_override;

/// The factory exported by the single shipped component.
struct ToolOverride;
impl Extension for ToolOverride {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        tool_override::factory(api)
    }
}

/// Immutable test input, also used as the controlled command's access log.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/tests/tool_override_fixture.txt"
);

#[test]
fn tool_override_registered_callbacks_follow_host_paths() -> Result<(), String> {
    super::block_on(async {
        let host = super::controlled::Controlled::default();
        host.observe().actions.value =
            json!({"config.get_agent_dir":"/controlled-agent", "paths.resolve":FIXTURE});
        let exports = crate::component_adapter::Exports::new(host.clone());
        exports.start::<ToolOverride>().await?;
        let key = host.observed().callback("tool read")?;
        for (path, input, expected) in [
            (
                FIXTURE,
                json!({"path":"relative.txt","offset":2,"limit":1}),
                json!({"lines":4}),
            ),
            (
                "/controlled/.env",
                json!({"path":".env"}),
                json!({"blocked":true}),
            ),
            (
                "/nonexistent-maestro-example/no-file.txt",
                json!({"path":"missing"}),
                json!({"error":true}),
            ),
        ] {
            host.observe().actions.value = json!({"paths.resolve":path, "paths.guest_path":path});
            let encoded = exports
                .invoke_tool(
                    key,
                    crate::bindings::exports::maestro::extension::guest::ToolInvocation {
                        tool_call_id: "native-call".into(),
                        params: input.to_string(),
                    },
                    ToolCapabilities {
                        ctx: host.context("/controlled"),
                        signal: None,
                        update: None,
                    },
                )
                .await?;
            let value = result_value(&encoded);
            assert_eq!(value["details"], expected);
            if path == FIXTURE {
                assert_eq!(value["content"][0]["text"], "second Ω");
            }
        }
        host.observe().actions.value =
            json!({"is_whitespace_char":false, "paths.guest_path":FIXTURE});
        let key = host.observed().callback("command read-log")?;
        exports
            .invoke_command(key, String::new(), host.command_context("/controlled"))
            .await?;
        let receipts = super::ui_host::receipts(&host.observed().transcript);
        assert_eq!(
            receipts.last().unwrap().operands,
            json!(["Recent file access:\nfirst\nsecond Ω\nthird\n", "Info"])
        );
        release_all(&host, &["tool read", "command read-log"]);
        Ok(())
    })
}

#[test]
fn tool_override_large_text_clips_at_whole_scalars() -> Result<(), String> {
    // The repository documentation supplies an immutable input larger than the clipping bound.
    super::block_on(async {
        let host = super::controlled::Controlled::default();
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/specs/maestro-port.md"
        );
        host.observe().actions.value = json!({"config.get_agent_dir":"/unused-agent", "paths.resolve":path, "paths.guest_path":path});
        let exports = crate::component_adapter::Exports::new(host.clone());
        exports.start::<ToolOverride>().await?;
        let key = host.observed().callback("tool read")?;
        let encoded = exports
            .invoke_tool(
                key,
                crate::bindings::exports::maestro::extension::guest::ToolInvocation {
                    tool_call_id: "large-native-call".into(),
                    params: json!({"path":"large"}).to_string(),
                },
                ToolCapabilities {
                    ctx: host.context("/controlled"),
                    signal: None,
                    update: None,
                },
            )
            .await?;
        let value = result_value(&encoded);
        let text = value["content"][0]["text"].as_str().unwrap();
        assert!(text.ends_with("\n\n[Output truncated at 50KB]"));
        assert!(text.len() <= 51_228);
        release_all(&host, &["tool read", "command read-log"]);
        Ok(())
    })
}
