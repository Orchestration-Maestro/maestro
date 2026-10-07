use std::rc::Rc;

use maestro_extensions_wasm::{ExtensionEvent, ExtensionFactory, ExtensionReturn, SessionEvent};

#[test]
fn compacted_entry_author_types_compile() {
    let _factory: ExtensionFactory = Rc::new(|api| {
        ExtensionReturn::Immediate(api.on(
            "session_compact",
            Rc::new(|event, _context| {
                ExtensionReturn::Future(Box::pin(async move {
                    if let ExtensionEvent::SessionEvent(SessionEvent::SessionCompactEvent(event)) =
                        event
                    {
                        let entry = &event.compaction_entry;
                        assert_eq!(entry.event_type(), "compaction");
                        let _: &str = &entry.summary;
                        let _: f64 = entry.tokens_before;
                        let _: bool = event.from_extension;
                    }
                    Ok(None)
                }))
            }),
        ))
    });
}

#[test]
fn compaction_preparation_author_types_compile() {
    use maestro_extensions_wasm::{
        CompactionResult, ExtensionEventResult, ModelRegistry, ReadonlySessionManager,
        SessionBeforeCompactResult,
    };
    use serde_json::Value;

    let _factory: ExtensionFactory = Rc::new(|api| {
        ExtensionReturn::Immediate(api.on(
            "session_before_compact",
            Rc::new(|event, context| {
                ExtensionReturn::Future(Box::pin(async move {
                    if let ExtensionEvent::SessionEvent(SessionEvent::SessionBeforeCompactEvent(
                        event,
                    )) = event
                    {
                        let preparation = &event.preparation;
                        let _: &Vec<Value> = &preparation.messages_to_summarize;
                        let _: &Vec<Value> = &preparation.turn_prefix_messages;
                        let _: &Vec<Value> = &event.branch_entries;
                        let _: bool = preparation.is_split_turn;
                        let _: f64 = preparation.tokens_before;
                        let _: &str = &preparation.first_kept_entry_id;
                        let _: Rc<dyn ReadonlySessionManager> = context.session_manager()?;
                        let _: Rc<dyn ModelRegistry> = context.model_registry()?;
                        let _ = <dyn ReadonlySessionManager>::get_entries;
                        let _ = <dyn ModelRegistry>::get_api_key_and_headers;
                        let summary = preparation
                            .messages_to_summarize
                            .iter()
                            .filter(|message| message["role"] == "user")
                            .map(|message| {
                                let content = message["content"]
                                    .as_str()
                                    .map(|text| {
                                        String::from_utf16_lossy(
                                            &text.encode_utf16().take(100).collect::<Vec<_>>(),
                                        )
                                    })
                                    .unwrap_or_else(|| "[complex]".into());
                                format!("- {content}")
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        return Ok(Some(ExtensionEventResult::SessionBeforeCompactResult(
                            SessionBeforeCompactResult {
                                cancel: None,
                                compaction: Some(CompactionResult {
                                    summary: format!("User requests:\n{summary}"),
                                    first_kept_entry_id: preparation.first_kept_entry_id.clone(),
                                    tokens_before: preparation.tokens_before,
                                    details: None,
                                }),
                            },
                        )));
                    }
                    Ok(None)
                }))
            }),
        ))
    });
}
