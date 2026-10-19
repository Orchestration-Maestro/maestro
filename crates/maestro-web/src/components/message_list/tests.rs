//! Count actual preparation-owner visits without shipped instrumentation.
use std::cell::Cell;

thread_local! {
    /// Index and ordered preparation visits on this test's execution thread.
    static VISITS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}
/// Observe each input admitted to the real result-index pass.
pub(super) fn index_visit() {
    VISITS.with(|visits| {
        let (index, prepare) = visits.get();
        visits.set((index + 1, prepare));
    });
}
/// Observe each input admitted to the real ordered-preparation pass.
pub(super) fn prepare_visit() {
    VISITS.with(|visits| {
        let (index, prepare) = visits.get();
        visits.set((index, prepare + 1));
    });
}
/// Take this test thread's actual owner work, never another parallel test's work.
fn take() -> (usize, usize) {
    VISITS.with(|visits| visits.replace((0, 0)))
}
/// Build interleaved emitted rows and tool results with duplicate last-write-wins IDs.
#[cfg(not(target_arch = "wasm32"))]
fn messages(count: usize) -> Vec<super::AgentMessage<super::BrowserMessages>> {
    use std::sync::{Arc, RwLock};
    (0..count)
        .map(|index| {
            if index % 2 == 0 {
                super::AgentMessage::User(Arc::new(RwLock::new(maestro_app::UserMessage {
                    content: maestro_app::UserContent::Text(format!("row {index}")),
                    timestamp: 0.,
                })))
            } else {
                super::AgentMessage::ToolResult(Arc::new(RwLock::new(
                    maestro_app::ToolResultMessage {
                        tool_call_id: "duplicate".into(),
                        tool_name: "controlled".into(),
                        content: vec![],
                        details: Some(serde_json::json!(index)),
                        is_error: false,
                        timestamp: 0.,
                    },
                )))
            }
        })
        .collect()
}
/// N and 2N owner visits stay linear; sharing a built index never traverses history again.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn stable_rows_use_linear_result_index() {
    use std::rc::Rc;
    for count in [20, 40] {
        let messages = messages(count);
        let host = Rc::new(crate::components::download_button::tests::Host::default());
        let mut list = super::MessageList::new(host);
        take();
        list.refresh(&messages).unwrap();
        assert_eq!(take(), (count, count));
        assert_eq!(list.items.len(), count / 2);
        assert_eq!(list.results.len(), 1);
        assert_eq!(
            list.results["duplicate"].details,
            Some(serde_json::json!(count - 1))
        );
        let shared = &list.results;
        for _ in &messages {
            assert_eq!(
                shared.get("duplicate").unwrap().details,
                Some(serde_json::json!(count - 1))
            );
        }
        assert_eq!(take(), (0, 0), "shared index control performs no new scan");
        for _ in &messages {
            list.refresh(&messages).unwrap();
        }
        assert_eq!(
            take(),
            (count * count, count * count),
            "repeated-whole-list negative control is quadratic"
        );
        list.refresh(&messages).unwrap();
        assert_eq!(
            take(),
            (count, count),
            "the next admitted refresh builds once again"
        );
    }
}
