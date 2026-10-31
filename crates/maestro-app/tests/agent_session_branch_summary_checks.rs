//! A branch summary requested through tree navigation goes through the session's stream
//! function, so its request settings and attribution match the session's other requests.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    HarnessOptions, Scripted, harness, on_task_set, start_prompt,
};
use maestro_session::session_manager::{NavigateTreeOptions, ReadonlySessionManager};

#[test]
fn branch_summary_request_reaches_the_session_stream_function() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            responses: Some(vec![
                Scripted::text("first answer"),
                Scripted::text("second answer"),
                Scripted::text("summary from the session stream"),
            ]),
            ..HarnessOptions::default()
        })
        .await;
        for text in ["first", "second"] {
            start_prompt(&h, text).await.unwrap().unwrap();
        }
        let root = h
            .session
            .session_manager()
            .borrow()
            .get_entries()
            .iter()
            .find(|entry| entry["type"] == "message")
            .map(|entry| entry["id"].as_str().unwrap().to_owned())
            .unwrap();
        let result = h
            .session
            .navigate_tree(
                &root,
                NavigateTreeOptions {
                    summarize: Some(true),
                    ..NavigateTreeOptions::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(
            h.requests.0.borrow().len(),
            3,
            "the summary request was counted"
        );
        let summary = result.summary_entry.expect("a summary").summary;
        assert!(
            summary.contains("summary from the session stream"),
            "{summary}"
        );
    });
}
