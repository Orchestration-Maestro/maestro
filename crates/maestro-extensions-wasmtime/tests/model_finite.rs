//! Nonfinite authored option data never reaches the registered native adapter.
#![cfg(test)]
use maestro_models::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
/// A dispatched request is a validation failure, regardless of the returned setup error.
#[derive(Default)]
struct Count(AtomicUsize);
impl<E: ProviderExecution, O> Invoke<E, ProviderCall<E, O>> for Count {
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, _: ProviderCall<E, O>) -> Self::Return {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(DiagnosticErrorInfo::error("unexpected native dispatch"))
    }
}
#[test]
fn model_imports_reject_nonfinite_options_before_dispatch() {
    let count = Arc::new(Count::default());
    register_api_provider(
        ApiProvider {
            api: "component-finite-options".into(),
            stream: count.clone(),
            stream_simple: count.clone(),
        },
        Some("component-finite-options".into()),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<Local>(
            std::rc::Rc::new(|_| panic!("no tails")),
            std::rc::Rc::new(|_| panic!("no errors")),
        );
        let (_owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let command = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        let output = command("model-finite".into(), runner.create_command_context())
            .await
            .unwrap_err()
            .to_string();
        let output: Vec<String> =
            serde_json::from_str(&output).unwrap_or_else(|_| panic!("{output}"));
        assert_eq!(output.len(), 42);
        assert!(
            output
                .iter()
                .all(|error| error == "extension wrote a non-finite number (Infinity or NaN)")
        );
        assert_eq!(count.0.load(Ordering::SeqCst), 0);
        bus.clear();
    });
    unregister_api_providers("component-finite-options");
}
