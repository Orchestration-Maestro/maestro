//! An authored editor's synchronous operations share their original callback emitter.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::json;

use super::basic_ui::{install, operate};
use super::scenario::Driver;

/// A normal factory uses the same emitter for editor calls and keeps the edited component.
async fn editor(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    assert_eq!(
        operate(driver, json!({ "op": "authoredEditorContract" })).await?,
        json!({ "ok": true })
    );
    assert_eq!(driver.widget("editor-contract", 24).await?, ["after"]);
    let effects: Vec<_> = driver
        .bus_emissions()
        .into_iter()
        .filter(|(channel, _)| channel.starts_with("editor-"))
        .collect();
    assert_eq!(
        effects,
        [
            ("editor-text".to_owned(), json!("")),
            ("editor-change".to_owned(), json!("λ\n雪")),
            ("editor-change".to_owned(), json!("λ\n雪!")),
            ("editor-submit".to_owned(), json!("λ\n雪!")),
            ("editor-callback-present".to_owned(), json!(true)),
            (
                "editor-optionals".to_owned(),
                json!([true, true, true, true, true, true, true, true])
            ),
            ("editor-text".to_owned(), json!("λ\n雪!")),
            ("editor-callback-present".to_owned(), json!(false)),
        ]
    );
    operate(
        driver,
        json!({ "op": "acquire", "call": { "op": "setWidget", "key": "editor-contract" } }),
    )
    .await?;
    driver.release_all().await;
    Ok(())
}

#[test]
fn authored_editor_preserves_text_callbacks_and_optional_absence() -> Result<(), String> {
    on_both_adapters!(editor)
}
