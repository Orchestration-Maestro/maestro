//! Native observations of the shared routing core.
use super::*;
use maestro_app::browser::components::sandbox::sandbox_runtime_provider::{
    SandboxRespond, SandboxRuntimeProvider,
};
use serde_json::Value;
use std::{cell::RefCell, rc::Rc, sync::Arc};

#[test]
fn router_sender_gate_rejects_foreign_and_unbound_sources() {
    let registry = Registry::<Shared, u8>::default();
    let trace = Arc::new(std::sync::Mutex::new(Vec::new()));
    registry.register(
        "victim".into(),
        Rc::new(RefCell::new(vec![Arc::new(provider(&trace, "provider"))])),
        vec![traced_consumer(trace.clone(), "consumer", false)],
    );
    registry.register("other".into(), Rc::new(RefCell::new(vec![])), vec![]);
    registry.set_frame("other", 2);
    for source in [None, Some(9)] {
        assert!(
            registry
                .window_context(Some("victim"), source.as_ref(), |frame| Some(*frame))
                .is_none()
        );
    }
    registry.set_frame("victim", 1);
    for source in [None, Some(9), Some(2)] {
        assert!(
            registry
                .window_context(Some("victim"), source.as_ref(), |frame| Some(*frame))
                .is_none()
        );
    }
    assert!(trace.lock().unwrap().is_empty());
    let admitted = registry
        .window_context(Some("victim"), Some(&1), |frame| Some(*frame))
        .unwrap();
    finish(admitted.dispatch(message(), respond())).unwrap();
    assert_eq!(*trace.lock().unwrap(), ["provider", "consumer"]);
    registry.set_frame("victim", 2);
    assert!(
        registry
            .window_context(Some("victim"), Some(&1), |frame| Some(*frame))
            .is_none()
    );
    assert!(
        registry
            .window_context(Some("victim"), Some(&2), |frame| Some(*frame))
            .is_some()
    );
}

#[test]
fn router_registration_emptiness_drives_listener_lifetime() {
    let registry = Registry::<Value, u8>::default();
    assert!(registry.is_empty());
    registry.register("one".into(), Rc::new(RefCell::new(vec![])), vec![]);
    assert!(!registry.is_empty());
    registry.register("two".into(), Rc::new(RefCell::new(vec![])), vec![]);
    registry.register("one".into(), Rc::new(RefCell::new(vec![])), vec![]);
    registry.set_frame("unknown", 4);
    registry.unregister("unknown");
    registry.unregister("one");
    assert!(!registry.is_empty());
    registry.unregister("two");
    assert!(registry.is_empty());
    registry.register("again".into(), Rc::new(RefCell::new(vec![])), vec![]);
    assert!(!registry.is_empty());
}

#[test]
fn router_sandbox_ids_are_matched_without_coercion() {
    let registry = Registry::<Value, u8>::default();
    for id in ["", "null", "0", "false", "[object Object]", "one"] {
        registry.register(id.into(), Rc::new(RefCell::new(vec![])), vec![]);
    }
    for id in [
        Value::Null,
        Value::Bool(false),
        Value::from(0),
        serde_json::json!({}),
        serde_json::json!([]),
        Value::from(""),
        Value::from("missing"),
    ] {
        assert!(registry.context(id.as_str()).is_none());
    }
    assert!(registry.context(None).is_none());
    assert!(registry.context(Some("one")).is_some());
}

/// Shared native test value without a transport copy.
#[derive(Clone)]
struct Shared(Arc<std::sync::Mutex<Value>>);
impl SandboxValue for Shared {
    fn format_repl_value(&self) -> Result<std::borrow::Cow<'_, str>, DiagnosticErrorInfo> {
        Ok("test".into())
    }
}
/// Consumer with immediate caller effects and an owned result.
struct Consumer(Box<dyn Fn(Shared) -> Result<(), DiagnosticErrorInfo>>);
impl MessageConsumer<Shared> for Consumer {
    fn handle_message(&self, message: Shared) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        let result = (self.0)(message);
        Box::pin(async move { result })
    }
}
/// Erase local callbacks before sharing their allocation identity.
fn consumer(
    action: impl Fn(Shared) -> Result<(), DiagnosticErrorInfo> + 'static,
) -> Arc<dyn MessageConsumer<Shared>> {
    let value: Box<dyn MessageConsumer<Shared>> = Box::new(Consumer(Box::new(action)));
    value.into()
}
/// Run a deterministic ready continuation, never an external executor.
fn finish<T>(future: impl std::future::Future<Output = T>) -> T {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
    {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => panic!("continuation has not been released"),
    }
}
/// Fresh shared message.
fn message() -> Shared {
    Shared(Arc::new(std::sync::Mutex::new(Value::from(0))))
}
/// Empty response callback.
fn respond() -> SandboxRespond<Shared> {
    Arc::new(|_| Ok(()))
}

#[test]
fn router_consumer_identity_preserves_live_order() {
    let registry = Rc::new(Registry::<Shared, u8>::default());
    let trace = Rc::new(RefCell::new(Vec::new()));
    let second: Arc<dyn MessageConsumer<Shared>> = consumer({
        let trace = trace.clone();
        move |_| {
            trace.borrow_mut().push("equal");
            Ok(())
        }
    });
    let removed = second.clone();
    let weak = Rc::downgrade(&registry);
    let added = consumer({
        let trace = trace.clone();
        move |_| {
            trace.borrow_mut().push("added");
            Ok(())
        }
    });
    let first: Arc<dyn MessageConsumer<Shared>> = consumer({
        let trace = trace.clone();
        let second = second.clone();
        move |_| {
            trace.borrow_mut().push("first");
            let registry = weak.upgrade().unwrap();
            let context = registry.context(Some("one")).unwrap();
            if trace.borrow().len() == 1 {
                context.remove(&removed);
                context.add(added.clone());
                context.add(second.clone());
                let first = context.consumers.borrow()[0].as_ref().unwrap().clone();
                context.remove(&first);
                context.add(first);
            }
            Ok(())
        }
    });
    let equal: Arc<dyn MessageConsumer<Shared>> = consumer({
        let trace = trace.clone();
        move |_| {
            trace.borrow_mut().push("equal");
            Ok(())
        }
    });
    registry.register(
        "one".into(),
        Rc::new(RefCell::new(vec![])),
        vec![first.clone(), first, second, equal],
    );
    let context = registry.context(Some("one")).unwrap();
    finish(context.dispatch(message(), respond())).unwrap();
    assert_eq!(
        *trace.borrow(),
        ["first", "equal", "added", "equal", "first"]
    );
}

/// Provider with a controllable completion signal.
struct Provider {
    /// Shared observations independent of registry mutation.
    trace: Arc<std::sync::Mutex<Vec<&'static str>>>,
    /// Distinguishing callback label.
    label: &'static str,
    /// Release flag for a deliberately suspended callback.
    release: Arc<std::sync::atomic::AtomicBool>,
    /// Callback failure branch.
    fail: bool,
    /// Edit the retained message before its successor reads it.
    edit: bool,
}
impl SandboxRuntimeProvider<Shared> for Provider {
    fn get_data(&self) -> Result<indexmap::IndexMap<String, Shared>, DiagnosticErrorInfo> {
        Ok(indexmap::IndexMap::new())
    }
    fn get_runtime(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(String::new())
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(String::new())
    }
    fn handle_message(
        &self,
        message: Shared,
        _respond: SandboxRespond<Shared>,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        self.trace.lock().unwrap().push(self.label);
        if self.edit {
            *message.0.lock().unwrap() = Value::from(2);
        }
        if self.label == "observe" {
            assert_eq!(*message.0.lock().unwrap(), Value::from(2));
        }
        let release = self.release.clone();
        let fail = self.fail;
        Box::pin(async move {
            std::future::poll_fn(|_| {
                release
                    .load(std::sync::atomic::Ordering::SeqCst)
                    .then_some(())
                    .map_or(std::task::Poll::Pending, std::task::Poll::Ready)
            })
            .await;
            if fail {
                Err(DiagnosticErrorInfo::error("callback failed"))
            } else {
                Ok(())
            }
        })
    }
}
/// Default provider retains the optional no-op handler.
struct DefaultProvider;
impl SandboxRuntimeProvider<Shared> for DefaultProvider {
    fn get_data(&self) -> Result<indexmap::IndexMap<String, Shared>, DiagnosticErrorInfo> {
        Ok(indexmap::IndexMap::new())
    }
    fn get_runtime(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(String::new())
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(String::new())
    }
}
/// Ready provider with a distinct trace label.
fn provider(trace: &Arc<std::sync::Mutex<Vec<&'static str>>>, label: &'static str) -> Provider {
    Provider {
        trace: trace.clone(),
        label,
        release: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        fail: false,
        edit: false,
    }
}

#[test]
fn router_dispatch_runs_providers_before_live_consumers() {
    let registry = Registry::<Shared, u8>::default();
    let trace = Arc::new(std::sync::Mutex::new(Vec::new()));
    let first = provider(&trace, "first");
    let release = first.release.clone();
    release.store(false, std::sync::atomic::Ordering::SeqCst);
    let providers: Providers<Shared> = Rc::new(RefCell::new(vec![
        Arc::new(first),
        Arc::new(provider(&trace, "replaced")),
    ]));
    let consumer: Arc<dyn MessageConsumer<Shared>> = consumer({
        let trace = trace.clone();
        move |_| {
            trace.lock().unwrap().push("consumer");
            Ok(())
        }
    });
    registry.register("one".into(), providers.clone(), vec![consumer]);
    let context = registry.context(Some("one")).unwrap();
    let mut pending = Box::pin(context.dispatch(message(), respond()));
    assert!(
        pending
            .as_mut()
            .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
            .is_pending()
    );
    assert_eq!(*trace.lock().unwrap(), ["first"]);
    providers.borrow_mut()[1] = Arc::new(DefaultProvider);
    providers
        .borrow_mut()
        .push(Arc::new(provider(&trace, "appended")));
    release.store(true, std::sync::atomic::Ordering::SeqCst);
    finish(pending).unwrap();
    assert_eq!(*trace.lock().unwrap(), ["first", "appended", "consumer"]);
}

/// Record one consumer invocation and optionally reject it.
fn traced_consumer(
    trace: Arc<std::sync::Mutex<Vec<&'static str>>>,
    label: &'static str,
    fail: bool,
) -> Arc<dyn MessageConsumer<Shared>> {
    consumer(move |_| {
        trace.lock().unwrap().push(label);
        if fail {
            Err(DiagnosticErrorInfo::error("consumer failed"))
        } else {
            Ok(())
        }
    })
}

#[test]
fn router_dispatch_stops_at_the_failing_callback() {
    for branch in 0..3 {
        let trace = Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = Registry::<Shared, u8>::default();
        let mut first = provider(&trace, "provider");
        first.fail = branch == 1;
        let consumers = ["consumer", "later"]
            .map(|label| traced_consumer(trace.clone(), label, branch == 2 && label == "consumer"));
        registry.register(
            "one".into(),
            Rc::new(RefCell::new(vec![Arc::new(first)])),
            consumers.into(),
        );
        let context = registry.context(Some("one")).unwrap();
        let result = finish(context.dispatch(message(), respond()));
        assert_eq!(result.is_err(), branch != 0);
        let expected: &[&str] = match branch {
            1 => &["provider"],
            2 => &["provider", "consumer"],
            _ => &["provider", "consumer", "later"],
        };
        assert_eq!(*trace.lock().unwrap(), expected);
        // A failure stops one delivery, not subsequent messages.
        assert_eq!(
            finish(context.dispatch(message(), respond())).is_err(),
            branch != 0
        );
        assert_eq!(&trace.lock().unwrap()[expected.len()..], expected);
    }
}

#[test]
fn router_callbacks_share_the_same_message_handle() {
    let registry = Registry::<Shared, u8>::default();
    let trace = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut editor = provider(&trace, "editor");
    editor.edit = true;
    let original = message();
    let consumer: Arc<dyn MessageConsumer<Shared>> = consumer({
        let original = original.clone();
        move |value| {
            assert!(Arc::ptr_eq(&original.0, &value.0));
            assert_eq!(*value.0.lock().unwrap(), Value::from(2));
            Ok(())
        }
    });
    registry.register(
        "one".into(),
        Rc::new(RefCell::new(vec![
            Arc::new(editor),
            Arc::new(provider(&trace, "observe")),
        ])),
        vec![consumer],
    );
    finish(
        registry
            .context(Some("one"))
            .unwrap()
            .dispatch(original.clone(), respond()),
    )
    .unwrap();
    assert_eq!(*original.0.lock().unwrap(), Value::from(2));
}

#[test]
fn router_registry_replacement_keeps_inflight_context() {
    for replace in [false, true] {
        let registry = Registry::<Shared, u8>::default();
        let trace = Arc::new(std::sync::Mutex::new(Vec::new()));
        let first = provider(&trace, "provider");
        let release = first.release.clone();
        release.store(false, std::sync::atomic::Ordering::SeqCst);
        let consumer: Arc<dyn MessageConsumer<Shared>> = consumer({
            let trace = trace.clone();
            move |_| {
                trace.lock().unwrap().push("old");
                Ok(())
            }
        });
        registry.register(
            "one".into(),
            Rc::new(RefCell::new(vec![Arc::new(first)])),
            vec![consumer],
        );
        let context = registry.context(Some("one")).unwrap();
        let mut pending = Box::pin(context.dispatch(message(), respond()));
        assert!(
            pending
                .as_mut()
                .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
                .is_pending()
        );
        registry.unregister("one");
        if replace {
            let consumer = traced_consumer(trace.clone(), "new", false);
            registry.register("one".into(), Rc::new(RefCell::new(vec![])), vec![consumer]);
        }
        release.store(true, std::sync::atomic::Ordering::SeqCst);
        finish(pending).unwrap();
        assert_eq!(*trace.lock().unwrap(), ["provider", "old"]);
        if replace {
            finish(
                registry
                    .context(Some("one"))
                    .unwrap()
                    .dispatch(message(), respond()),
            )
            .unwrap();
            assert_eq!(&trace.lock().unwrap()[2..], ["new"]);
        } else {
            assert!(registry.context(Some("one")).is_none());
        }
    }
}

#[test]
fn router_responses_follow_the_retained_iframe_slot() {
    let registry = Registry::<Shared, u8>::default();
    let trace = Arc::new(std::sync::Mutex::new(Vec::new()));
    let first = provider(&trace, "provider");
    let release = first.release.clone();
    release.store(false, std::sync::atomic::Ordering::SeqCst);
    registry.register(
        "one".into(),
        Rc::new(RefCell::new(vec![Arc::new(first)])),
        vec![],
    );
    registry.set_frame("one", 1);
    let context = registry.context(Some("one")).unwrap();
    let slot = context.frame.clone();
    let mut pending = Box::pin(context.dispatch(message(), respond()));
    assert!(
        pending
            .as_mut()
            .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
            .is_pending()
    );
    registry.set_frame("one", 2);
    release.store(true, std::sync::atomic::Ordering::SeqCst);
    finish(pending).unwrap();
    assert_eq!(slot.borrow().as_deref(), Some(&2));
    registry.register("one".into(), Rc::new(RefCell::new(vec![])), vec![]);
    registry.set_frame("one", 3);
    assert_eq!(slot.borrow().as_deref(), Some(&2));
    assert_eq!(
        registry
            .context(Some("one"))
            .unwrap()
            .frame
            .borrow()
            .as_deref(),
        Some(&3)
    );
}
