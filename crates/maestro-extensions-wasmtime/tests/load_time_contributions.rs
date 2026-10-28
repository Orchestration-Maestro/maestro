//! A built component offers load-time data through the original native API's admission window.
use maestro_extensions::{
    Local, create_event_bus_for, create_extension_runtime, load_extension_from_factory,
    types::FlagValue,
};
use serde_json::json;
use std::rc::Rc;

/// The component retains exact JSON data and cannot contribute outside base loading.
#[test]
fn guest_load_time_inputs_reach_the_original_base_runtime() {
    super::components::local(async {
        let path = std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap();
        let payload = json!({
            "packageSources":[{"source":"./package","scope":"project"}],
            "baseSystemPrompt":"base system", "baseAppendText":["first", "second"],
            "modelFiles":["./models.json"], "baseLayers":[]
        });
        let bus = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no errors")),
        );
        let runtime = create_extension_runtime::<Local>();
        runtime.flag_values.write().unwrap().insert(
            "load-time-inputs".into(),
            FlagValue::String(payload.to_string()),
        );
        runtime.begin_load_time(vec![path.clone()]);
        let owner = load_extension_from_factory(
            crate::artifact(path.clone()),
            "/work".into(),
            bus.for_extension(),
            runtime.clone(),
            Some(path.clone()),
        )
        .await
        .unwrap();
        assert_eq!(runtime.finish_load_time(), [(path.clone(), payload)]);
        assert_eq!(owner.read().unwrap().path, path);
        let rejected = load_extension_from_factory(
            crate::artifact(path.clone()),
            "/work".into(),
            bus.for_extension(),
            runtime,
            Some(path),
        )
        .await
        .err()
        .unwrap()
        .to_string();
        assert!(rejected.contains("base package loading"), "{rejected}");
    });
}
