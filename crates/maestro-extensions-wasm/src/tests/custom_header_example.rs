//! The shipped custom header exercised through its built component and retained view.
use serde_json::json;
use wasmtime::component::Resource;

use super::documents::Answer;
use super::example_support::invoke_command_on_component;
use super::host::{Harness, borrow};
use super::host_family::guest::Capabilities;
use super::ui_host::receipts;

/// Starts the shipped component with a built-in header ready to be replaced.
fn start(has_ui: bool) -> Result<Harness, String> {
    let mut harness = Harness::example("custom_header")?;
    assert!(receipts(&harness.observed().transcript).is_empty());
    assert_eq!(
        harness.observed().registrations.commands,
        [(
            "builtin-header".to_owned(),
            Some("Restore built-in header with keybinding hints".into()),
            false
        )]
    );
    harness.observed().catalog.data.has_ui = has_ui;
    harness
        .observed()
        .actions
        .control(r#"{"value":{"version":"9.8.7-host"}}"#);
    harness.observed().ui.control(r#"{"header":"ready"}"#);
    Ok(harness)
}

/// Delivers a session start to the registration exported by the component.
fn session_start(harness: &mut Harness, reason: &str) -> Result<Answer, String> {
    let key = harness
        .callback("event session_start")
        .map_err(|e| e.to_string())?;
    let ctx = harness.ordinary("/controlled").map_err(|e| e.to_string())?;
    let exports = harness.exports.clone();
    let event = json!({"type":"session_start", "reason":reason});
    let delivered = super::block_on(harness.store.run_concurrent(async |accessor| {
        exports
            .call_invoke_event(
                accessor,
                borrow(key),
                event.to_string(),
                Capabilities { ctx, signal: None },
            )
            .await
    }))
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())??;
    Answer::of(super::host_family::delivery(delivered))
}

/// Renders or invalidates the retained header with a host-owned emitter.
fn view(harness: &mut Harness, width: Option<u32>) -> Result<Vec<String>, String> {
    let component = harness
        .observed()
        .ui
        .header
        .custom
        .as_ref()
        .ok_or("no custom header")?
        .component;
    let emitter = harness
        .store
        .data_mut()
        .table
        .push(super::bus_host::Emitter { key: 0 })
        .map_err(|e| e.to_string())?;
    let callback = Resource::new_borrow(emitter.rep());
    let lines = if let Some(width) = width {
        super::block_on(
            harness
                .exports
                .func_component_render()
                .call_async(&mut harness.store, (borrow(component), width, callback)),
        )
        .map_err(|e| e.to_string())?
        .0?
    } else {
        super::block_on(
            harness
                .exports
                .func_component_invalidate()
                .call_async(&mut harness.store, (borrow(component), callback)),
        )
        .map_err(|e| e.to_string())?
        .0?;
        Vec::new()
    };
    harness
        .store
        .data_mut()
        .table
        .delete(emitter)
        .map_err(|e| e.to_string())?;
    Ok(lines)
}

/// Restores the header through its command and releases the view it replaced.
fn restore(harness: &mut Harness) -> Result<(), String> {
    let component = harness
        .observed()
        .ui
        .header
        .custom
        .as_ref()
        .map(|slot| slot.component);
    let key = harness
        .callback("command builtin-header")
        .map_err(|e| e.to_string())?;
    invoke_command_on_component(harness, key, "ignored arguments", false)?;
    if let Some(component) = component {
        super::block_on(harness.release(component)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Releases the event and command registrations.
fn finish(harness: &mut Harness) -> Result<(), String> {
    for name in ["event session_start", "command builtin-header"] {
        let key = harness.callback(name).map_err(|e| e.to_string())?;
        super::block_on(harness.release(key)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Every start reason installs the illustration and host version using the live supplied theme.
#[test]
fn custom_header_component_renders_host_version_with_live_theme() -> Result<(), String> {
    let mut harness = start(true)?;
    for reason in ["startup", "reload", "new", "resume", "fork"] {
        let event = json!({"type":"session_start", "reason":reason});
        assert_eq!(
            session_start(&mut harness, reason)?,
            Answer::returned(&event, None)
        );
        assert_eq!(harness.observed().ui.header.children, ["banner", "custom"]);
        for palette in ["dark", "alternate"] {
            harness.observed().themes.control(
                &json!({"theme":"dark", "palette":format!("custom-header-{palette}")}).to_string(),
            );
            let expected = vec![
                String::new(),
                format!("<{palette} accent>       ▄</accent>"),
                format!("<{palette} accent>      ▀█▀</accent>"),
                format!("<{palette} accent>       ▀</accent>"),
                format!("<{palette} accent>  █▄       ▄█</accent>"),
                format!("<{palette} accent>  █ ▀▄   ▄▀ █</accent>"),
                format!("<{palette} accent>  █   ▀█▀   █</accent>"),
                String::new(),
                format!(
                    "<{palette} muted>   Custom header</muted><{palette} dim> v9.8.7-host</dim>"
                ),
            ];
            for width in [1, 24, 80] {
                assert_eq!(view(&mut harness, Some(width))?, expected);
            }
            assert!(view(&mut harness, None)?.is_empty());
            assert_eq!(view(&mut harness, Some(80))?, expected);
        }
        restore(&mut harness)?;
    }
    finish(&mut harness)
}

/// The command restores the built-in slot before notifying, including without a custom header.
#[test]
fn custom_header_component_restores_builtin_header() -> Result<(), String> {
    let mut harness = start(true)?;
    session_start(&mut harness, "new")?;
    for _ in 0..2 {
        let mark = harness.observed().transcript.len();
        restore(&mut harness)?;
        assert!(harness.observed().ui.header.custom.is_none());
        assert_eq!(
            harness.observed().ui.header.children,
            ["banner", "built-in"]
        );
        let seen = receipts(&harness.observed().transcript[mark..]);
        assert_eq!(seen.len(), 2);
        assert_eq!(
            (seen[0].method.as_str(), &seen[0].operands),
            ("setHeader", &json!([false]))
        );
        assert_eq!(
            (seen[1].method.as_str(), &seen[1].operands),
            ("notify", &json!(["Built-in header restored", "Info"]))
        );
    }
    finish(&mut harness)
}

/// Headless startup asks for no UI, while the reset command still forwards its two requests.
#[test]
fn custom_header_component_skips_headless_startup() -> Result<(), String> {
    let mut harness = start(false)?;
    let event = json!({"type":"session_start", "reason":"new"});
    assert_eq!(
        session_start(&mut harness, "new")?,
        Answer::returned(&event, None)
    );
    assert!(receipts(&harness.observed().transcript).is_empty());
    assert!(harness.observed().ui.header.custom.is_none());
    restore(&mut harness)?;
    let seen = receipts(&harness.observed().transcript);
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].method, "setHeader");
    assert_eq!(seen[1].method, "notify");
    finish(&mut harness)
}
