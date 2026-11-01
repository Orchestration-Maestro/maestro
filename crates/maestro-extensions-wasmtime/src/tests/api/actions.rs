//! Scalar metadata and model admission use the same original runtime.
use super::api;
use crate::read_imports::ReadEndpoint;
use crate::{api_reads::Request, local_resources::Pump, tests::model_support};
use maestro_extensions::{Local, create_extension_runtime};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[test]
fn scalar_actions_keep_values_order_and_catchable_errors() {
    tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async {
        let runtime = create_extension_runtime::<Local>();
        let (_owner, api) = api(runtime.clone());
        let seen = Rc::new(RefCell::new(Vec::new()));
        {
            let mut actions = runtime.actions.write().unwrap();
            let output = seen.clone();
            actions.set_session_name = Rc::new(move |name| { output.borrow_mut().push(serde_json::json!(["name", name])); Ok(()) });
            actions.get_session_name = Rc::new(|| Ok(Some(String::new())));
            let output = seen.clone();
            actions.set_label = Rc::new(move |id, label| { output.borrow_mut().push(serde_json::json!(["label", id, label])); Ok(()) });
            let output = seen.clone();
            actions.set_active_tools = Rc::new(move |tools| { output.borrow_mut().push(serde_json::json!(["tools", tools])); Ok(()) });
            actions.get_all_tools = Rc::new(|| Ok(vec![serde_json::from_value(serde_json::json!({"name":" tool ","description":"","parameters":{"type":"object"},"sourceInfo":{"path":"p","source":"custom","scope":"project","origin":"package","baseDir":"base"}})).unwrap()]));
            actions.get_commands = Rc::new(|| Ok(vec![serde_json::from_value(serde_json::json!({"name":" command ","description":"","source":"custom-source","sourceInfo":{"path":"p","source":"custom","scope":"project","origin":"package","baseDir":"base"}})).unwrap()]));
            actions.get_thinking_level = Rc::new(|| Ok(maestro_models::ModelThinkingLevel::High));
            let output = seen.clone();
            actions.set_thinking_level = Rc::new(move |level| { output.borrow_mut().push(serde_json::json!(["thinking", level])); Err("selected failure".into()) });
        }
        let (pump, admission) = Pump::new(crate::tests::support::widgets());
        let endpoint = pump.registry().api(api).unwrap();
        let worker = std::thread::spawn(move || {
            endpoint.read(|reply| Request::SetSessionName(" raw\u{feff}\u{85} ".into(), reply)).unwrap();
            assert_eq!(endpoint.read(Request::SessionName).unwrap().as_deref(), Some(""));
            for label in [None, Some(String::new()), Some(" label ".into())] {
                endpoint.read(|reply| Request::Label(" id ".into(), label, reply)).unwrap();
            }
            endpoint.read(|reply| Request::SetActiveTools(vec![" z ".into(), "a".into(), "a".into()], reply)).unwrap();
            assert_eq!(serde_json::from_str::<serde_json::Value>(&endpoint.schema_listing().unwrap().0).unwrap(), serde_json::json!([{"name":" tool ","description":"","parameters":{"type":"object"},"sourceInfo":{"path":"p","source":"custom","scope":"project","origin":"package","baseDir":"base"},"schemaKinds":{}}]));
            assert_eq!(serde_json::from_str::<serde_json::Value>(&endpoint.read(Request::Commands).unwrap()).unwrap(), serde_json::json!([{"name":" command ","description":"","source":"custom-source","sourceInfo":{"path":"p","source":"custom","scope":"project","origin":"package","baseDir":"base"}}]));
            assert_eq!(endpoint.read(Request::Thinking).unwrap(), "\"high\"");
            assert_eq!(endpoint.read(|reply| Request::SetThinking("\"off\"".into(), reply)).unwrap_err(), "selected failure");
            assert!(endpoint.read(|reply| Request::SetThinking("invalid".into(), reply)).is_err());
        });
        drop(admission);
        pump.serve().await;
        worker.join().unwrap();
        assert_eq!(*seen.borrow(), [serde_json::json!(["name", " raw\u{feff}\u{85} "]), serde_json::json!(["label", " id ", null]), serde_json::json!(["label", " id ", ""]), serde_json::json!(["label", " id ", " label "]), serde_json::json!(["tools", [" z ", "a", "a"]]), serde_json::json!(["thinking", "off"])]);
    });
}

#[test]
fn admitted_model_selection_survives_rebind_and_retirement() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let runtime = create_extension_runtime::<Local>();
            let (_owner, api) = api(runtime.clone());
            let started = Rc::new(Cell::new(false));
            let (resume, resumed) = tokio::sync::oneshot::channel();
            let resumed = RefCell::new(Some(resumed));
            let selected = started.clone();
            runtime.actions.write().unwrap().set_model = Rc::new(move |model| {
                assert_eq!(model.read().unwrap().id, " selected ");
                selected.set(true);
                let resumed = resumed.borrow_mut().take().unwrap();
                Box::pin(async move {
                    resumed.await?;
                    Ok(false)
                })
            });
            let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
            let endpoint = pump.registry().api(api).unwrap();
            let model = model_support::model(" selected ").read().unwrap().clone();
            let reader = endpoint.clone();
            let worker = std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .build()
                    .unwrap()
                    .block_on(async move {
                        assert!(!reader.set_model(model).await.unwrap());
                        let later = model_support::model("later").read().unwrap().clone();
                        assert_eq!(reader.set_model(later).await.unwrap_err(), "retired model");
                    });
            });
            assert!(pump.answer_next().await);
            assert!(started.get());
            runtime.actions.write().unwrap().set_model = Rc::new(|_| Box::pin(async { Ok(true) }));
            runtime.invalidate(Some("retired model".into()));
            resume.send(()).unwrap();
            drop(endpoint);
            drop(admission);
            pump.serve().await;
            worker.join().unwrap();
        });
}

#[test]
fn thinking_levels_use_canonical_json_in_both_directions() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let runtime = create_extension_runtime::<Local>();
            let (_owner, api) = api(runtime.clone());
            let state = Rc::new(RefCell::new(maestro_extensions::ModelThinkingLevel::Off));
            let get = state.clone();
            runtime.actions.write().unwrap().get_thinking_level =
                Rc::new(move || Ok(get.borrow().clone()));
            let set = state.clone();
            runtime.actions.write().unwrap().set_thinking_level = Rc::new(move |level| {
                *set.borrow_mut() = level;
                Ok(())
            });
            let (pump, admission) = Pump::new(crate::tests::support::widgets());
            let endpoint = pump.registry().api(api).unwrap();
            let worker = std::thread::spawn(move || {
                assert_eq!(endpoint.read(Request::Thinking).unwrap(), "\"off\"");
                for level in ["off", "minimal", "low", "medium", "high", "xhigh"] {
                    let wire = serde_json::to_string(level).unwrap();
                    endpoint
                        .read(|reply| Request::SetThinking(wire.clone(), reply))
                        .unwrap();
                    assert_eq!(endpoint.read(Request::Thinking).unwrap(), wire);
                }
                for invalid in ["high", "null", "{}", "\"invalid\""] {
                    assert!(
                        endpoint
                            .read(|reply| Request::SetThinking(invalid.into(), reply))
                            .is_err()
                    );
                }
                assert_eq!(endpoint.read(Request::Thinking).unwrap(), "\"xhigh\"");
            });
            drop(admission);
            pump.serve().await;
            worker.join().unwrap();
            assert_eq!(
                *state.borrow(),
                maestro_extensions::ModelThinkingLevel::Xhigh
            );
        });
}
