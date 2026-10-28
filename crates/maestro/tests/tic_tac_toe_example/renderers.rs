//! Tool rows and custom messages rendered through the native retained callback bindings.
use maestro_app::{
    AgentSession, MessageRenderOptions, ToolRenderContext, ToolRenderEnvironment, ToolRenderPhase,
    ToolRenderResultLike, ToolRenderResultOptions, ToolRendererState,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Exercises both custom messages and both tool renderers, collapsed and expanded.
pub(super) fn exercise(session: &AgentSession, details: &Value) {
    messages(session, details);
    tools(session, details);
}

/// Invokes the two custom-message factories with independently supplied snapshots.
fn messages(session: &AgentSession, details: &Value) {
    let runner = session.extension_runner();
    for (name, title) in [
        ("tic-tac-toe-move", "Player X played"),
        ("tic-tac-toe-game-over", "Draw"),
    ] {
        let renderer = runner
            .get_message_renderer_call::<Value>(name)
            .expect("the shipped message renderer");
        let mut details = details.clone();
        if name.ends_with("game-over") {
            details["status"] = json!("draw");
        }
        let message = json!({"role":"custom","customType":name,"content":"fallback","display":true,"details":details,"timestamp":0});
        for expanded in [false, true] {
            let view = renderer
                .call(
                    Rc::new(serde_json::from_value(message.clone()).unwrap()),
                    MessageRenderOptions { expanded },
                    super::ui::theme(),
                )
                .unwrap()
                .unwrap();
            let text = maestro_app::strip_ansi(&view.try_render(80).unwrap().join("\n")).unwrap();
            assert!(text.contains(title));
            if expanded {
                assert!(text.contains('╳'));
                assert!(text.contains('◯'));
            }
        }
    }
}

/// Renders both tools through their retained call and result slots.
fn tools(session: &AgentSession, details: &Value) {
    let runner = session.extension_runner();
    for tool in runner.get_all_registered_tools() {
        let binding = tool.definition.renderer_binding().unwrap();
        let args = json!({"action":"play"});
        let rows = ToolRendererState::default();
        let context = context(&args, &rows);
        let call =
            binding.render_call.expect("call renderer")(Some(&args), &super::ui::theme(), &context)
                .unwrap();
        assert!(
            maestro_app::strip_ansi(&call.try_render(80).unwrap().join("\n"))
                .unwrap()
                .contains("tic_tac_toe")
        );
        let prepared = binding
            .render_result
            .expect("result renderer")
            .prepare(ToolRenderResultLike {
                content: &[],
                details: Some(details),
                terminate: None,
                is_error: Some(false),
            })
            .unwrap();
        for expanded in [false, true] {
            let view = prepared
                .render(
                    ToolRenderResultOptions {
                        expanded,
                        is_partial: false,
                    },
                    &super::ui::theme(),
                    &context,
                )
                .unwrap();
            let lines = view.try_render(80).unwrap();
            assert!(!lines.is_empty());
            if expanded {
                assert!(
                    maestro_app::strip_ansi(&lines.join("\n"))
                        .unwrap()
                        .contains('◯')
                );
            }
        }
    }
}

/// Supplies the ordinary retained row environment, not a replacement rendering engine.
fn context<'a>(
    args: &'a Value,
    rows: &'a ToolRendererState,
) -> ToolRenderContext<'a, ToolRendererState> {
    ToolRenderContext {
        args: Some(args),
        tool_call_id: "game-render".into(),
        invalidate: Rc::new(|| {}),
        environment: Rc::new(ToolRenderEnvironment {
            runtime: crate::tools_example::manual_runtime::ManualRuntime::new().handle(),
            readme_path: String::new(),
            live_theme: super::ui::theme(),
            highlighter: Rc::new(maestro_theme::SyntectHighlighter::new().unwrap()),
            terminal_image: maestro_tui::TerminalImage::default(),
        }),
        last_component: None,
        state: Rc::new(RefCell::new(rows.clone())),
        renderer_state: Some(rows),
        cwd: "/work".into(),
        phase: ToolRenderPhase {
            execution_started: true,
            args_complete: true,
            is_partial: false,
        },
        expanded: true,
        show_images: false,
        is_error: false,
    }
}

/// Error artifacts have no board, even when the user expands their result row.
pub(super) fn errors(session: &AgentSession) {
    for tool in session.extension_runner().get_all_registered_tools() {
        let args = json!({"action":"play"});
        let rows = ToolRendererState::default();
        let mut context = context(&args, &rows);
        context.is_error = true;
        let blocks = serde_json::from_value::<Vec<maestro_app::UserBlock>>(
            json!([{"type":"text","text":"It is not your turn."}]),
        )
        .unwrap();
        let details = json!({});
        let prepared = tool
            .definition
            .renderer_binding()
            .unwrap()
            .render_result
            .unwrap()
            .prepare(ToolRenderResultLike {
                content: &blocks,
                details: Some(&details),
                terminate: None,
                is_error: Some(true),
            })
            .unwrap();
        for expanded in [false, true] {
            let view = prepared
                .render(
                    ToolRenderResultOptions {
                        expanded,
                        is_partial: false,
                    },
                    &super::ui::theme(),
                    &context,
                )
                .unwrap();
            let text = maestro_app::strip_ansi(&view.try_render(80).unwrap().join("\n")).unwrap();
            assert!(text.contains("It is not your turn."), "{text}");
            assert!(text.contains('✗'));
            assert!(!text.contains('◯'));
        }
    }
}
