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
