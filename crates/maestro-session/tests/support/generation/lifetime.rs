//! Controlled retained-producer observations.
use super::*;
/// The producer retains its stream while compaction drops a pending sibling observation.
pub fn lifetime() {
    for case in corpus().lifetime {
        let failing = if case.failing == "history" {
            "history"
        } else {
            assert_eq!(case.failing, "prefix");
            "prefix"
        };
        let state = adapter(json!({}));
        let model = {
            let rows = corpus().rows;
            model(&rows[0], &state.api)
        };
        let events = Arc::new(Mutex::new(Vec::new()));
        let pending = Arc::new(Mutex::new(None));
        register_pending(&state, failing, &events, &pending);
        let preparation = preparation(
            &json!({"split":true,"messages":[{"role":"user","content":"history"}],"prefix":[{"role":"user","content":"prefix"}]}),
        );
        let future = compact(&preparation, &model, &options(&json!({})));
        assert_eq!(*events.lock().unwrap(), ["history-start", "prefix-start"]);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let error = runtime.block_on(future).unwrap_err();
        assert_eq!(error.message, case.error);
        events.lock().unwrap().push("compact-rejected".into());
        let stream = pending.lock().unwrap().take().unwrap();
        let observed = stream.result();
        let response = Arc::new(RwLock::new(
            serde_json::from_value(default_response()).unwrap(),
        ));
        stream.end(Some(Arc::clone(&response)));
        assert!(Arc::ptr_eq(&runtime.block_on(observed), &response));
        let sibling = if failing == "history" {
            "prefix"
        } else {
            "history"
        };
        events.lock().unwrap().push(format!("{sibling}-settled"));
        assert_eq!(*events.lock().unwrap(), case.events);
    }
}

/// Install controlled completion with a held stream on the successful side.
fn register_pending(
    state: &Adapter,
    failing: &'static str,
    events: &Arc<Mutex<Vec<String>>>,
    pending: &Arc<Mutex<Option<AssistantMessageEventStream>>>,
) {
    let (recorded, retained) = (Arc::clone(events), Arc::clone(pending));
    maestro_models::register_api_provider(
        ApiProvider {
            api: state.api.clone(),
            stream: Arc::new(|_, _, _| unreachable!()),
            stream_simple: Arc::new(move |_, context, _| {
                let context = serde_json::to_value(context).unwrap();
                let kind = if context["messages"][0]["content"][0]["text"]
                    .as_str()
                    .unwrap()
                    .contains("This is the PREFIX of a turn")
                {
                    "prefix"
                } else {
                    "history"
                };
                recorded.lock().unwrap().push(format!("{kind}-start"));
                let stream = AssistantMessageEventStream::new();
                if kind == failing {
                    let mut value = default_response();
                    value["stopReason"] = json!("error");
                    value["errorMessage"] = json!(format!("{kind} failure"));
                    stream.end(Some(Arc::new(RwLock::new(
                        serde_json::from_value(value).unwrap(),
                    ))));
                } else {
                    *retained.lock().unwrap() = Some(stream.clone());
                }
                Ok(stream)
            }),
        },
        Some(state.api.clone()),
    );
}
