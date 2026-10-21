//! Supplied-path snapshot fixtures and scoped diagnostics.
use maestro_workspace::persist::{SessionSnapshot, load};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

/// Isolated native fixture directory.
pub struct Directory(pub PathBuf);
impl Directory {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "maestro-snapshot-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub fn put(&self, value: &Value) -> PathBuf {
        let path = self.0.join("input.json");
        fs::write(&path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
        path
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

pub fn empty() -> SessionSnapshot {
    serde_json::from_value(json!({"version":1,"workspaces":[],"active":null,"selected":0,"sidebar_width":26,"sidebar_section_split":0.5})).unwrap()
}

pub fn read(value: &Value) -> Option<SessionSnapshot> {
    let dir = Directory::new();
    load(&dir.put(value))
}

/// One observed event's named diagnostic fields.
pub type Event = std::collections::BTreeMap<String, String>;

#[derive(Clone)]
struct Collector(std::sync::Arc<std::sync::Mutex<Vec<Event>>>);
impl tracing::Subscriber for Collector {
    fn register_callsite(
        &self,
        _: &'static tracing::Metadata<'static>,
    ) -> tracing::subscriber::Interest {
        tracing::subscriber::Interest::sometimes()
    }
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        let mut visitor = Visitor(Event::new());
        event.record(&mut visitor);
        self.0.lock().unwrap().push(visitor.0);
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}
struct Visitor(Event);
impl tracing::field::Visit for Visitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.insert(field.name().into(), format!("{value:?}"));
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(field.name().into(), value.into());
    }
}

pub fn capture<T>(action: impl FnOnce() -> T) -> (T, Vec<Event>) {
    let collector = Collector(std::sync::Arc::default());
    let result = maestro_test_tracing::with_default(collector.clone(), action);
    let events = std::mem::take(&mut *collector.0.lock().unwrap());
    (result, events)
}

pub fn assert_error(value: &Value, expected: &str) {
    let (result, events) = capture(|| read(value));
    assert!(result.is_none());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["err"], expected);
}

pub fn populated() -> Value {
    json!({
        "version": 1, "active": 0, "selected": 1,
        "sidebar_width": 29, "sidebar_section_split": 0.3,
        "collapsed_space_keys": [],
        "workspaces": [{
            "id": "workspace", "custom_name": "Named workspace", "identity_cwd": "/checkout",
            "public_pane_numbers": {"9": 47, "20": 81}, "next_public_pane_number": 82,
            "public_tab_numbers": [17], "next_public_tab_number": 18, "active_tab": 0,
            "tabs": [{"custom_name": "Named tab", "layout": {"Split": {
                "direction": "Horizontal", "ratio": 0.25, "first": {"Pane": 9}, "second": {"Pane": 20}
            }}, "panes": {"9": {"cwd": "/first", "label": "First"}, "20": {"cwd": "/second"}},
            "zoomed": true, "focused": 20, "root_pane": 9}]
        }]
    })
}

pub fn history() -> maestro_workspace::persist::SessionHistorySnapshot {
    serde_json::from_value(json!({"version":1,"workspaces":[
        {"tabs":[{"panes":{"9":{"ansi":"\u{1b}[31mprivate screen\n\u{1b}[0m","lines":12}}},{"panes":{"20":{"ansi":"second tab","lines":7}}}]},
        {"tabs":[{"panes":{"9":{"ansi":"other workspace","lines":3}}}]}
    ]})).unwrap()
}

/// Observes the JSON bytes emitted by the native serde writer.
pub fn encoded(value: &impl serde::Serialize) -> Value {
    serde_json::from_slice(&serde_json::to_vec_pretty(value).unwrap()).unwrap()
}
