//! List preparation reads each history entry a bounded number of times.
use super::{MessageList, index};
use crate::components::messages::{AssistantMessage, ToolResults};
use maestro_app::{AgentMessage, NoCustomMessages, ToolResultMessage};
use serde_json::json;
use std::{
    cell::Cell,
    collections::HashSet,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// One history entry.
type Entry = super::Message<NoCustomMessages>;

/// An assistant calling `id`.
fn assistant(id: &str) -> Entry {
    let message = json!({
        "role":"assistant", "content":[{"type":"toolCall","id":id,"name":"probe","arguments":{}}],
        "api":"controlled", "provider":"controlled", "model":"controlled",
        "usage":maestro_app::Usage::default(), "stopReason":"stop", "timestamp":0,
    });
    AgentMessage::Assistant(Arc::new(RwLock::new(
        serde_json::from_value(message).unwrap(),
    )))
}
/// A result envelope for `id`.
fn result(id: &str) -> Entry {
    AgentMessage::ToolResult(Arc::new(RwLock::new(ToolResultMessage {
        tool_call_id: id.into(),
        tool_name: "probe".into(),
        content: Vec::new(),
        details: None,
        is_error: false,
        timestamp: 0.,
        usage: None,
    })))
}
/// The original handle of a result entry.
fn handle(entry: &Entry) -> &Arc<RwLock<ToolResultMessage>> {
    match entry {
        AgentMessage::ToolResult(result) => result,
        _ => unreachable!(),
    }
}

#[test]
fn message_list_indexing_stays_linear() {
    for count in [8, 16, 32] {
        // Every call has two results; the later one is selected.
        let history: Vec<Entry> = (0..count)
            .flat_map(|index| {
                let id = index.to_string();
                [result(&id), assistant(&id), result(&id)]
            })
            .collect();
        let visited = Cell::new(0);
        let results = Rc::new(index(
            history.iter().inspect(|_| visited.set(visited.get() + 1)),
        ));
        assert_eq!(visited.get(), history.len(), "one index pass");
        assert_eq!(results.len(), count);
        for (index, chunk) in history.chunks(3).enumerate() {
            assert!(Arc::ptr_eq(&results[&index.to_string()], handle(&chunk[2])));
        }
        let list = MessageList::<NoCustomMessages> {
            messages: Arc::new(RwLock::new(history.clone())),
            ..MessageList::default()
        };
        let reads = Cell::new(0);
        let mut position = 0;
        let rows = list
            .emit(
                || {
                    reads.set(reads.get() + 1);
                    let next = history.get(position).cloned();
                    position += 1;
                    next
                },
                &results,
            )
            .unwrap();
        assert_eq!(reads.get(), history.len() + 1, "one ordered emission pass");
        assert_eq!(rows.len(), count);
        assert!(visited.get() + reads.get() <= 2 * history.len() + 1);
        assert_eq!(list.prepare().unwrap().len(), count);
        // Negative control: searching the history for each call's result is quadratic.
        let rescans = Cell::new(0);
        for index in 0..count {
            let id = index.to_string();
            let found = (history.iter().rev())
                .inspect(|_| rescans.set(rescans.get() + 1))
                .find(|entry| matches!(entry, AgentMessage::ToolResult(result) if result.read().unwrap().tool_call_id == id));
            assert!(found.is_some());
        }
        assert!(rescans.get() > 2 * history.len());
    }
}

#[test]
fn message_list_forwards_original_operands_with_stable_flags() {
    let tools = Rc::new(RwLock::new(Vec::new()));
    let pending = Rc::new(HashSet::from(["0".to_owned()]));
    let action: Rc<dyn Fn()> = Rc::new(|| {});
    for streaming in [false, true] {
        let list = MessageList::<NoCustomMessages> {
            messages: Arc::new(RwLock::new(vec![assistant("0"), result("0")])),
            tools: Rc::clone(&tools),
            pending_tool_calls: Some(Rc::clone(&pending)),
            is_streaming: streaming,
            on_cost_click: Some(Rc::clone(&action)),
        };
        let rows = list.prepare().unwrap();
        let row = rows[0].downcast_ref::<AssistantMessage>().unwrap();
        assert!(!row.is_streaming && !row.hide_tool_calls);
        assert_eq!(row.hide_pending_tool_calls, streaming);
        assert!(Rc::ptr_eq(row.tools.as_ref().unwrap(), &tools));
        assert!(Rc::ptr_eq(
            row.pending_tool_calls.as_ref().unwrap(),
            &pending
        ));
        assert!(Rc::ptr_eq(row.on_cost_click.as_ref().unwrap(), &action));
        let AgentMessage::Assistant(original) = &list.messages.read().unwrap()[0] else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(row.message.as_ref().unwrap(), original));
        let Some(ToolResults::Live(results)) = &row.tool_results_by_id else {
            panic!("stable rows read the live result index")
        };
        let history = list.messages.read().unwrap();
        assert!(Arc::ptr_eq(&results["0"], handle(&history[1])));
    }
}
