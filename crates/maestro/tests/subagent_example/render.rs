//! The actual component's retained call and result renderers.
use super::*;
use maestro_app::{ToolRenderResultLike, ToolRenderResultOptions, ToolRendererState};

/// Supplies literal child observations, not a second rendering implementation.
fn details(mode: &str) -> Value {
    let result = json!({"agent":"role","agentSource":"user","task":"Full delegated task","exitCode":0,
        "messages":[{"role":"assistant","content":[
            {"type":"toolCall","name":"bash","arguments":{"command":"printf controlled"}},
            {"type":"toolCall","name":"read","arguments":{"path":"file.rs","offset":2,"limit":3}},
            {"type":"toolCall","name":"write","arguments":{"path":"out.rs","content":"one\ntwo"}},
            {"type":"toolCall","name":"edit","arguments":{"path":"edit.rs"}},
            {"type":"toolCall","name":"ls","arguments":{}},
            {"type":"toolCall","name":"find","arguments":{"pattern":"*.rs"}},
            {"type":"toolCall","name":"grep","arguments":{"pattern":"needle"}},
            {"type":"toolCall","name":"custom","arguments":{"value":true}},
            {"type":"text","text":"first line\nsecond line\nthird line\nfourth line"}]}],
        "stderr":"","usage":{"input":1500,"output":15000,"cacheRead":1_200_000,"cacheWrite":0,"cost":0.125,"contextTokens":1800,"turns":2},"model":"controlled-model","step":1});
    json!({"mode":mode,"agentScope":"user","projectAgentsDir":null,"results":[result]})
}

/// Renders a complete result with each supplied view option through the native binding.
fn rendered(session: &AgentSession, args: &Value, details: &Value, expanded: bool) -> String {
    let binding = session
        .get_tool_definition("subagent")
        .unwrap()
        .definition
        .renderer_binding()
        .unwrap();
    let rows = ToolRendererState::default();
    let context = crate::tic_tac_toe_example::renderers::context(args, &rows);
    let prepared = binding
        .render_result
        .expect("subagent result renderer")
        .prepare(ToolRenderResultLike {
            content: &[],
            details: Some(details),
            terminate: None,
            is_error: Some(false),
        })
        .unwrap();
    let view = prepared
        .render(
            ToolRenderResultOptions {
                expanded,
                is_partial: false,
            },
            &crate::support::theme(),
            &context,
        )
        .unwrap();
    maestro_app::strip_ansi(&view.try_render(120).unwrap().join("\n")).unwrap()
}

/// Checks compact tool arguments, retained Markdown and per-agent/aggregate usage.
fn result_views(session: &AgentSession) {
    let args = json!({"agent":"role","task":"inspect"});
    for mode in ["single", "chain", "parallel"] {
        let details = details(mode);
        let collapsed = rendered(session, &args, &details, false);
        assert!(collapsed.contains("role"), "{collapsed}");
        assert!(!collapsed.contains("fourth line"), "{collapsed}");
        let expanded = rendered(session, &args, &details, true);
        for expected in [
            "Full delegated task",
            "$ printf controlled",
            "read file.rs:2-4",
            "write out.rs (2 lines)",
            "edit edit.rs",
            "ls .",
            "find *.rs in .",
            "grep /needle/ in .",
            "custom",
            "fourth line",
            "↑1.5k",
            "↓15k",
            "R1.2M",
            "$0.1250",
            "controlled-model",
        ] {
            assert!(
                expanded.contains(expected),
                "missing {expected}: {expanded}"
            );
        }
    }
}

#[test]
fn subagent_renderers_show_calls_full_outputs_usage_and_live_progress() {
    run(|local| {
        Box::pin(async move {
            let root = Root::new();
            let session = loaded(&root, local).await;
            let args = json!({"chain":[{"agent":"role","task":"inspect {previous}"},{"agent":"next","task":"continue"}],"agentScope":"both"});
            let rows = ToolRendererState::default();
            let context = crate::tic_tac_toe_example::renderers::context(&args, &rows);
            let binding = session
                .get_tool_definition("subagent")
                .unwrap()
                .definition
                .renderer_binding()
                .unwrap();
            let call = binding.render_call.expect("subagent call renderer")(
                Some(&args),
                &crate::support::theme(),
                &context,
            )
            .unwrap();
            let text = maestro_app::strip_ansi(&call.try_render(120).unwrap().join("\n")).unwrap();
            assert!(text.contains("chain (2 steps) [both]"), "{text}");
            assert!(!text.contains("{previous}"), "{text}");
            result_views(&session);
            let mut pending = details("parallel");
            pending["results"][0]["exitCode"] = json!(-1);
            let text = rendered(&session, &json!({}), &pending, true);
            assert!(text.contains("0/1 done, 1 running"), "{text}");
        })
    });
}
