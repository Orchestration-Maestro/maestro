//! The next turn of a run reads the session's current tools and system prompt, while the
//! request already sent keeps its payload.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Harness, callback_tool, start_prompt, system_prompt_extension,
};
use indexmap::IndexMap;
use maestro_app::AgentSession;
use maestro_app::test_support::{HarnessOptions, Scripted, harness, on_task_set};
use maestro_models::Local;
use maestro_test_directory::Scratch;
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

/// The session a tool callback reaches once the harness exists.
type LateSession = Rc<RefCell<Weak<AgentSession<Local>>>>;

/// A session whose `echo` tool deactivates `other` and records the system prompt it then
/// sees, over a scripted run of one tool turn followed by a text answer.
async fn deactivating(
    factories: Vec<maestro_extensions::ExtensionFactory<Local>>,
) -> (Harness, Rc<RefCell<Vec<String>>>) {
    let late = LateSession::default();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (reach, record) = (Rc::clone(&late), Rc::clone(&seen));
    let echo = callback_tool("echo", move || {
        let session = reach.borrow().upgrade().unwrap();
        session
            .set_active_tools_by_name(&["echo".to_owned()])
            .unwrap();
        record.borrow_mut().push(session.system_prompt());
    });
    let h = harness(
        Scratch::new().unwrap(),
        HarnessOptions {
            factories,
            responses: Some(vec![Scripted::echo_call("x"), Scripted::text("done")]),
            base_tools_override: Some(IndexMap::from([
                ("echo".to_owned(), echo),
                ("other".to_owned(), callback_tool("other", || {})),
            ])),
            initial_active_tool_names: Some(vec!["echo".to_owned(), "other".to_owned()]),
            ..HarnessOptions::default()
        },
    )
    .await
    .unwrap();
    *late.borrow_mut() = Rc::downgrade(&h.session);
    (h, seen)
}

/// The names of the tools request `index` carried.
fn tool_names(h: &Harness, index: usize) -> Vec<String> {
    let requests = h.requests.0.borrow();
    let tools = requests[index].tools.as_deref().unwrap_or_default();
    tools.iter().map(|tool| tool.name.clone()).collect()
}

/// The system prompt request `index` carried.
fn system_prompt(h: &Harness, index: usize) -> Option<String> {
    h.requests.0.borrow()[index].system_prompt.clone()
}

#[test]
fn tools_changed_during_a_run_reach_the_next_request_only() {
    on_task_set(|_| async {
        let (h, _) = deactivating(Vec::new()).await;
        start_prompt(&h, "go").await.unwrap().unwrap();
        assert_eq!(
            tool_names(&h, 0),
            ["echo", "other"],
            "the sent request keeps its tools"
        );
        assert_eq!(
            tool_names(&h, 1),
            ["echo"],
            "the next request sees the new set"
        );
    })
    .unwrap();
}

#[test]
fn run_system_prompt_survives_a_tool_refresh() {
    on_task_set(|_| async {
        let (h, seen) = deactivating(vec![system_prompt_extension("RUN")]).await;
        start_prompt(&h, "go").await.unwrap().unwrap();
        assert_eq!(*seen.borrow(), ["RUN"], "the refresh kept the run's prompt");
        assert_eq!(system_prompt(&h, 0).as_deref(), Some("RUN"));
        assert_eq!(system_prompt(&h, 1).as_deref(), Some("RUN"));
    })
    .unwrap();
}

#[test]
fn empty_run_system_prompt_is_an_override() {
    on_task_set(|_| async {
        let (h, seen) = deactivating(vec![system_prompt_extension("")]).await;
        start_prompt(&h, "go").await.unwrap().unwrap();
        assert_eq!(*seen.borrow(), [""], "the refresh kept the empty prompt");
        assert_eq!(system_prompt(&h, 0).as_deref(), Some(""));
        assert_eq!(system_prompt(&h, 1).as_deref(), Some(""));
    })
    .unwrap();
}
