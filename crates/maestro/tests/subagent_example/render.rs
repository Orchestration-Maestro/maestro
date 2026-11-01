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
pub(super) fn rendered(
    session: &AgentSession,
    args: &Value,
    details: &Value,
    expanded: bool,
) -> String {
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
    #[cfg(unix)]
    {
        let home = std::env::var("HOME").unwrap();
        let sibling = format!("{home}-sibling/file.rs");
        let mut details = details("single");
        details["results"][0]["messages"][0]["content"][1]["arguments"]["path"] = json!(sibling);
        assert!(rendered(session, &args, &details, true).contains(&sibling));
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

#[test]
fn subagent_usage_rounds_decimal_ties_in_component_views() {
    run(|local| {
        Box::pin(async move {
            let root = Root::new();
            let session = loaded(&root, local).await;
            let mut details = details("single");
            details["results"][0]["usage"] = json!({"input":1250,"output":1_250_000,"cacheRead":0,"cacheWrite":0,"cost":0.03125,"contextTokens":1250,"turns":1});
            for expanded in [false, true] {
                let text = rendered(&session, &json!({}), &details, expanded);
                assert!(text.contains("↑1.3k ↓1.3M $0.0313 ctx:1.3k"), "{text}");
            }
        })
    });
}

#[test]
#[cfg(unix)]
fn subagent_tool_paths_keep_literal_backslashes_outside_home() {
    run(|local| {
        Box::pin(async move {
            let root = Root::new();
            let session = loaded(&root, local).await;
            let home = std::env::var("HOME").unwrap();
            for (path, expected) in [
                (home.clone(), "~".into()),
                (format!("{home}/file.rs"), "~/file.rs".into()),
                (format!("{home}\\file.rs"), format!("{home}\\file.rs")),
            ] {
                let mut details = details("single");
                details["results"][0]["messages"][0]["content"][1]["arguments"]["path"] =
                    json!(path);
                let text = rendered(&session, &json!({}), &details, true);
                assert!(text.contains(&format!("read {expected}:2-4")), "{text}");
            }
        })
    });
}

#[test]
fn subagent_call_previews_bound_single_parallel_and_chain_arguments() {
    run(|local| {
        Box::pin(async move {
            let root = Root::new();
            let session = loaded(&root, local).await;
            let binding = session
                .get_tool_definition("subagent")
                .unwrap()
                .definition
                .renderer_binding()
                .unwrap();
            for (args, expected, preview, hidden) in [
                (
                    json!({"agent":"solo","task":"s".repeat(61)}),
                    "solo [user]",
                    format!("{}...", "s".repeat(60)),
                    "s".repeat(61),
                ),
                (
                    json!({"tasks":[{"agent":"one","task":"p".repeat(41)},{"agent":"two","task":"second"}]}),
                    "parallel (2 tasks) [user]",
                    format!("{}...", "p".repeat(40)),
                    "p".repeat(41),
                ),
                (
                    json!({"chain":[{"agent":"one","task":"{previous} first"},{"agent":"two","task":"second"},{"agent":"three","task":"third"},{"agent":"hidden","task":"secret"}]}),
                    "chain (4 steps) [user]",
                    "... +1 more".into(),
                    "secret".into(),
                ),
                (
                    json!({"tasks":vec![json!({"agent":"role","task":"p".repeat(41)});4]}),
                    "parallel (4 tasks) [user]",
                    "... +1 more".into(),
                    "p".repeat(41),
                ),
            ] {
                let rows = ToolRendererState::default();
                let context = crate::tic_tac_toe_example::renderers::context(&args, &rows);
                let call = binding.render_call.as_ref().unwrap()(
                    Some(&args),
                    &crate::support::theme(),
                    &context,
                )
                .unwrap();
                let text =
                    maestro_app::strip_ansi(&call.try_render(120).unwrap().join("\n")).unwrap();
                assert!(text.contains(expected), "{text}");
                assert!(text.contains(&preview), "{text}");
                assert!(!text.contains(&hidden), "{text}");
                assert!(!text.contains("{previous}"), "{text}");
            }
        })
    });
}
