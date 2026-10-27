//! Retained terminal observations through both adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{
    basic_ui::{install, operate},
    scenario::Driver,
};
use serde_json::json;

/// A retained terminal reads its host's current counter, not a guest-side counter.
async fn redraws(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    assert_eq!(
        operate(driver, json!({"op":"tuiFactory"})).await?,
        json!({"ok": true})
    );
    assert_eq!(driver.widget("terminal", 7).await?, ["redraws=17"]);
    assert_eq!(driver.widget("terminal", 11).await?, ["redraws=17"]);
    driver.release_all().await;
    Ok(())
}

#[test]
fn retained_tui_reads_the_host_full_redraw_count() -> Result<(), String> {
    on_both_adapters!(redraws)
}

/// The focus flag exists while false, changes synchronously and emits a zero-width marker.
async fn focus(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    assert_eq!(
        operate(driver, json!({"op":"focusContract"})).await?,
        json!({"ok":["field\u{1b}_maestro:c\u{7} [true, false, true]"]})
    );
    assert_eq!(driver.widget("focus", 10).await?, ["field []"]);
    driver.release_all().await;
    Ok(())
}

#[test]
fn authored_focus_keeps_setter_order_and_cursor_marker() -> Result<(), String> {
    on_both_adapters!(focus)
}

/// A host editor remains focusable after conversion to a component.
async fn editors(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    assert_eq!(
        operate(driver, json!({"op":"hostEditorFocus"})).await?,
        json!({"ok":true})
    );
    assert!(!driver.widget("editor-focus", 20).await?.is_empty());
    driver.release_all().await;
    Ok(())
}

#[test]
fn host_editors_keep_focus_through_component_views() -> Result<(), String> {
    on_both_adapters!(editors)
}

/// The same retained host record is visible to both guest aliases.
async fn overlay_options(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    assert_eq!(
        operate(driver, json!({"op":"overlayOptions"})).await?,
        json!({"ok":"overlay options shared"})
    );
    driver.release_all().await;
    Ok(())
}

#[test]
fn overlay_options_preserve_placement_and_shared_mutation() -> Result<(), String> {
    on_both_adapters!(overlay_options)
}

/// Completion keeps the guest value's identity and ignores a later completion.
async fn custom(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    for (asynchronous, answer) in [(false, "custom ready done"), (true, "custom async done")] {
        assert_eq!(
            operate(driver, json!({"op":"custom", "asynchronous":asynchronous})).await?,
            json!({"ok":answer})
        );
    }
    driver.release_all().await;
    Ok(())
}

#[test]
fn custom_component_preserves_opaque_done_for_ready_and_async_factories() -> Result<(), String> {
    on_both_adapters!(custom)
}
