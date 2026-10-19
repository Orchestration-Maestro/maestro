//! Stable transcript behavior through the public aggregate and owned recipes.
#![cfg(test)]
use crate::{
    attachment_support::Frame, effects_support, message_renderer_support, renderer_recipes,
};
use maestro_app::browser::components::messages::{
    ArtifactAction, CustomAgentMessages as BrowserMessages,
};
use maestro_app::{AgentMessage, CustomAgentMessages};
use maestro_web::{ArtifactMessage, MessageList, MessageListProps, register_message_renderer};
use maestro_web::{browser::attachment_content::AttachmentHost, tools::types::ToolRenderView};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock, atomic::AtomicU32},
};

/// Artifact filtering precedes even a registered callback; adjacent open roles still render.
#[test]
fn stable_rows_filter_artifacts_before_custom_renderers() {
    let observed = Rc::new(RefCell::new(Vec::new()));
    for role in ["artifact", "transcript-adjacent"] {
        let observed = Rc::clone(&observed);
        register_message_renderer(
            role,
            Rc::new(message_renderer_support::Renderer::<
                BrowserMessages<message_renderer_support::NativeMessage>,
            >(Rc::new(move |message| {
                let AgentMessage::Custom(value) = message else {
                    unreachable!()
                };
                observed
                    .borrow_mut()
                    .push(value.read().unwrap().role().to_owned());
                Ok(Box::new(renderer_recipes::Caption(
                    "adjacent recipe".into(),
                )))
            }))),
        );
    }
    let messages = [
        AgentMessage::Custom(Arc::new(RwLock::new(BrowserMessages::Artifact(
            ArtifactMessage {
                action: ArtifactAction::Create,
                filename: "note.txt".into(),
                content: None,
                title: None,
                timestamp: "0".into(),
            },
        )))),
        AgentMessage::Custom(Arc::new(RwLock::new(BrowserMessages::Other(
            message_renderer_support::NativeMessage {
                role: "transcript-adjacent".into(),
                handle: Arc::new(AtomicU32::new(7)),
            },
        )))),
    ];
    let host: Rc<dyn AttachmentHost> = Rc::new(effects_support::Host::default());
    let mut list = MessageList::new(Rc::clone(&host));
    let mut frame = Frame::default();
    draw(
        &mut frame,
        &host,
        &mut list,
        MessageListProps {
            messages: &messages,
            ..Default::default()
        },
        &crate::highlighter_support::NoHighlight,
    );
    assert_eq!(*observed.borrow(), ["transcript-adjacent"]);
    assert_eq!(frame.texts(), ["adjacent recipe"]);
}

/// Draw a list through the existing native frame and controlled reading host.
fn draw<C: CustomAgentMessages + 'static>(
    frame: &mut Frame,
    host: &Rc<dyn AttachmentHost>,
    list: &mut MessageList<C>,
    props: MessageListProps<'_, C>,
    engine: &dyn maestro_theme::SyntaxHighlighter,
) {
    frame.run(vec![], |ctx, i18n, presentation| {
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("transcript"),
            egui::UiBuilder::new()
                .max_rect(ctx.content_rect())
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        let output = list.render(
            &mut ui,
            props,
            ToolRenderView {
                host,
                i18n,
                presentation,
                highlighter: engine,
                modal: None,
            },
        );
        assert!(output.overlays.is_empty());
        output.response.unwrap();
    });
}

/// Builtin assistant dispatch pairs the last result while adjacent rows keep emitted order.
#[test]
fn stable_rows_pair_results_and_preserve_emitted_order() {
    let host: Rc<dyn AttachmentHost> = Rc::new(effects_support::Host::default());
    let mut list = MessageList::<message_renderer_support::NativeMessage>::new(Rc::clone(&host));
    let mut frame = Frame::default();
    draw(
        &mut frame,
        &host,
        &mut list,
        MessageListProps::default(),
        &crate::highlighter_support::NoHighlight,
    );
    assert!(frame.texts().is_empty());
    let engine = Rc::new(crate::recording_highlighter::Highlighter::default());
    let seen = Rc::new(RefCell::new(Vec::new()));
    register_pair_observer(&engine, &seen);
    let messages = paired_history();
    draw(
        &mut frame,
        &host,
        &mut list,
        MessageListProps {
            messages: &messages,
            ..Default::default()
        },
        engine.as_ref(),
    );
    assert_eq!(*seen.borrow(), [2.]);
    assert_eq!(frame.texts(), ["paired last"]);
    assert_eq!(
        *engine.requests.borrow(),
        [
            ("user\n".into(), "javascript".into()),
            ("attachment-user\n".into(), "javascript".into()),
            ("user\n".into(), "javascript".into())
        ]
    );
}

/// Supplied roles override builtins without serializing custom handles or losing empty slots.
#[test]
fn registered_roles_override_builtins_without_serializing_native_handles() {
    use std::sync::atomic::Ordering;
    let handles = Rc::new(RefCell::new(Vec::new()));
    register_overrides(&handles);
    let handle = Arc::new(AtomicU32::new(7));
    let messages = [
        AgentMessage::User(Arc::new(RwLock::new(maestro_app::UserMessage {
            content: maestro_app::UserContent::Text("builtin must not render".into()),
            timestamp: 0.,
        }))),
        AgentMessage::ToolResult(Arc::new(RwLock::new(renderer_recipes::message(None)))),
        AgentMessage::Custom(Arc::new(RwLock::new(BrowserMessages::Other(
            message_renderer_support::NativeMessage {
                role: "transcript-native".into(),
                handle: Arc::clone(&handle),
            },
        )))),
    ];
    let effects: Rc<dyn AttachmentHost> = Rc::new(effects_support::Host::default());
    let mut list = MessageList::new(Rc::clone(&effects));
    let mut frame = Frame::default();
    draw(
        &mut frame,
        &effects,
        &mut list,
        MessageListProps {
            messages: &messages,
            ..Default::default()
        },
        &crate::highlighter_support::NoHighlight,
    );
    assert_eq!(frame.texts(), ["", "result override", "native override"]);
    assert_eq!(handles.borrow().len(), 1);
    handles.borrow()[0].store(9, Ordering::Relaxed);
    assert_eq!(handle.load(Ordering::Relaxed), 9);
    // Removing the empty first recipe shifts the emitted positions.
    draw(
        &mut frame,
        &effects,
        &mut list,
        MessageListProps {
            messages: &messages[1..],
            ..Default::default()
        },
        &crate::highlighter_support::NoHighlight,
    );
    assert_eq!(frame.texts(), ["result override", "native override"]);
}

/// Record result selection at the actual child renderer, after the preceding user row.
fn register_pair_observer(
    engine: &Rc<crate::recording_highlighter::Highlighter>,
    seen: &Rc<RefCell<Vec<f64>>>,
) {
    use maestro_web::tools::types::{ToolRendererBinding, ToolResultMessageView};
    let callback_engine = Rc::clone(engine);
    let captured = Rc::clone(seen);
    maestro_web::register_tool_renderer(
        "transcript-pair",
        ToolRendererBinding::from_raw(Rc::new(renderer_recipes::Renderer(Rc::new(
            move |_: Option<&maestro_app::JsonObject>,
                  result: Option<ToolResultMessageView<'_>>,
                  streaming| {
                assert!(!streaming);
                assert_eq!(callback_engine.requests.borrow().len(), 1);
                let result = result.unwrap();
                captured.borrow_mut().push(result.timestamp());
                Ok(renderer_recipes::content("paired last", true))
            },
        )))),
    );
}

/// Ordered repeated users, an attachment user, duplicate results and an unregistered role.
fn paired_history() -> Vec<AgentMessage<BrowserMessages<message_renderer_support::NativeMessage>>> {
    use maestro_app::{UserContent, UserMessage};
    let assistant = serde_json::from_value(serde_json::json!({
        "content":[{"type":"toolCall","id":"original call","name":"transcript-pair","arguments":{}}],
        "api":"openai-completions","provider":"openai","model":"controlled","timestamp":0,"stopReason":"stop",
        "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}}
    })).unwrap();
    let user = AgentMessage::User(Arc::new(RwLock::new(UserMessage {
        content: UserContent::Text("```javascript\nuser\n```".into()),
        timestamp: 0.,
    })));
    let mut first = renderer_recipes::message(None);
    first.timestamp = 1.;
    let mut last = first.clone();
    last.timestamp = 2.;
    let attachment_user = AgentMessage::Custom(Arc::new(RwLock::new(
        BrowserMessages::UserWithAttachments(maestro_web::UserMessageWithAttachments {
            content: UserContent::Text("```javascript\nattachment-user\n```".into()),
            timestamp: 0.,
            attachments: Some(vec![]),
        }),
    )));
    let unknown = AgentMessage::Custom(Arc::new(RwLock::new(BrowserMessages::Other(
        message_renderer_support::NativeMessage {
            role: "transcript-unknown".into(),
            handle: Arc::new(AtomicU32::new(0)),
        },
    ))));
    vec![
        unknown,
        user.clone(),
        AgentMessage::Assistant(Arc::new(RwLock::new(assistant))),
        AgentMessage::ToolResult(Arc::new(RwLock::new(first))),
        AgentMessage::ToolResult(Arc::new(RwLock::new(last))),
        attachment_user,
        user,
    ]
}

/// Keep override recipe preparation outside drawing and retain the original native handle.
fn register_overrides(handles: &Rc<RefCell<Vec<Arc<AtomicU32>>>>) {
    type Custom = BrowserMessages<message_renderer_support::NativeMessage>;
    for role in ["user", "toolResult", "transcript-native"] {
        let handles = Rc::clone(handles);
        register_message_renderer(
            role,
            Rc::new(message_renderer_support::Renderer::<Custom>(Rc::new(
                move |message| Ok(override_recipe(message, &handles)),
            ))),
        );
    }
}

/// Observe the original custom handle and distinguish every overridden builtin.
fn override_recipe(
    message: &AgentMessage<BrowserMessages<message_renderer_support::NativeMessage>>,
    handles: &Rc<RefCell<Vec<Arc<AtomicU32>>>>,
) -> Box<dyn maestro_web::tools::types::ToolContent> {
    let caption = match message {
        AgentMessage::User(_) => "",
        AgentMessage::ToolResult(_) => "result override",
        AgentMessage::Custom(value) => {
            let value = value.read().unwrap();
            let BrowserMessages::Other(native) = &*value else {
                unreachable!()
            };
            handles.borrow_mut().push(Arc::clone(&native.handle));
            "native override"
        }
        AgentMessage::Assistant(_) => unreachable!(),
    };
    Box::new(renderer_recipes::Caption(caption.into()))
}

/// Appending, replacing and truncating retain only the unaffected positional owners.
#[test]
fn stable_rows_keep_positional_widget_lifetimes() {
    let recorded = Rc::new(crate::attachment_support::Host::default());
    let host: Rc<dyn AttachmentHost> = recorded.clone();
    let mut list = MessageList::<message_renderer_support::NativeMessage>::new(Rc::clone(&host));
    let mut frame = Frame::default();
    let mut messages = vec![lifetime_user("first"), lifetime_user("second")];
    let mut refresh = |messages: &[_]| {
        recorded.surfaces.borrow_mut().clear();
        draw(
            &mut frame,
            &host,
            &mut list,
            MessageListProps {
                messages,
                ..Default::default()
            },
            &crate::highlighter_support::NoHighlight,
        );
        published_roots(&recorded)
    };
    let initial = refresh(&messages);
    assert_eq!(initial.len(), 4);
    messages.push(lifetime_user("third"));
    let appended = refresh(&messages);
    assert_eq!(appended.len(), 6);
    assert_same_roots(&initial, &appended[..4]);
    assert!(recorded.effects.borrow().is_empty());
    // Repeated publication must still draw every completed row.
    assert_same_roots(&appended, &refresh(&messages));
    register_message_renderer(
        "transcript-replacement",
        Rc::new(message_renderer_support::Renderer::<
            BrowserMessages<message_renderer_support::NativeMessage>,
        >(Rc::new(|_| {
            Ok(Box::new(renderer_recipes::Caption("replacement".into())))
        }))),
    );
    messages[1] = AgentMessage::Custom(Arc::new(RwLock::new(BrowserMessages::Other(
        message_renderer_support::NativeMessage {
            role: "transcript-replacement".into(),
            handle: Arc::new(AtomicU32::new(0)),
        },
    ))));
    let replaced = refresh(&messages);
    assert_same_roots(&appended[..2], &replaced[..2]);
    assert_same_roots(&appended[4..], &replaced[2..]);
    assert_removed_roots(&recorded, &appended[2..4]);
    messages.truncate(2);
    let truncated = refresh(&messages);
    assert_same_roots(&initial[..2], &truncated);
    assert_removed_roots(&recorded, &appended[4..]);
    drop(list);
    assert_removed_roots(&recorded, &initial[..2]);
}

/// Create ordinary builtin content without registering a role override.
fn lifetime_user(
    text: &str,
) -> AgentMessage<BrowserMessages<message_renderer_support::NativeMessage>> {
    AgentMessage::User(Arc::new(RwLock::new(maestro_app::UserMessage {
        content: maestro_app::UserContent::Text(text.into()),
        timestamp: 0.,
    })))
}

/// Compare both the stable component identity and its owned native mount recipe.
fn assert_same_roots(
    before: &[(
        egui::Id,
        Rc<maestro_web::browser::attachment_content::ReadingContent>,
    )],
    after: &[(
        egui::Id,
        Rc<maestro_web::browser::attachment_content::ReadingContent>,
    )],
) {
    assert_eq!(before.len(), after.len());
    for ((old_id, old_root), (new_id, new_root)) in before.iter().zip(after) {
        assert_eq!(old_id, new_id);
        assert!(Rc::ptr_eq(old_root, new_root));
    }
}

/// Consume lifecycle observations so an unrelated row release cannot pass unnoticed.
fn assert_removed_roots(
    host: &crate::attachment_support::Host,
    roots: &[(
        egui::Id,
        Rc<maestro_web::browser::attachment_content::ReadingContent>,
    )],
) {
    let mut actual = host.effects.take();
    let mut expected = roots
        .iter()
        .map(|(id, _)| format!("remove:{id:?}"))
        .collect::<Vec<_>>();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}

/// Read only actually published native owners, excluding hidden measurement candidates.
fn published_roots(
    host: &crate::attachment_support::Host,
) -> Vec<(
    egui::Id,
    Rc<maestro_web::browser::attachment_content::ReadingContent>,
)> {
    host.surfaces
        .borrow()
        .iter()
        .filter_map(|surface| match &surface.content {
            crate::attachment_support::Content::Reading(
                content,
                maestro_web::browser::attachment_content::ReadingPhase::Publish,
            ) => Some((surface.id, Rc::clone(content))),
            _ => None,
        })
        .collect()
}
