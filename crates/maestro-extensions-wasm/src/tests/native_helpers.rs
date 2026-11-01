//! The same author source and corpus select controlled imports without an author change.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{Extension, ExtensionAPI, ExtensionFuture};
use serde_json::{Value, json};
#[path = "../../examples/helper_capabilities.rs"]
mod author;
/// The unchanged compiled author factory, selected against controlled imports.
struct Helpers;
impl Extension for Helpers {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        author::factory(api)
    }
}
/// The identical corpus the production host integration test consumes.
fn corpus() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../../maestro-extensions-wasmtime/tests/fixtures/helper_corpus.json"
    ))
    .unwrap()
}
/// Supplies selected replies, not a second helper implementation.
fn replies(query: &str) -> Value {
    json!({"apply_background_to_line":"x   ","fuzzy.project":!query.is_empty(),"fuzzy.filter":[1,2,0]})
}
#[test]
fn helper_author_swaps_to_controlled_imports() -> Result<(), String> {
    super::block_on(async {
        let (host, exports) = super::example_support::start::<Helpers>().await?;
        let key = host.observed().callback("command helpers")?;
        for row in corpus()
            .iter()
            .filter(|row| row["test"] == "helper_adapter_swaps_without_author_changes")
        {
            host.observe().actions.value = replies(row["args"][0].as_str().unwrap());
            let result = exports
                .invoke_command(key, row.to_string(), host.command_context("/helpers"))
                .await
                .unwrap_err();
            let value: Value =
                serde_json::from_str(&result).unwrap_or_else(|error| panic!("{error}: {result}"));
            assert_eq!(value, row["ok"]);
        }
        super::example_support::release_all(&host, &["command helpers"]);
        Ok(())
    })
}
#[test]
fn helper_fuzzy_filter_rejects_out_of_range_host_positions() -> Result<(), String> {
    super::block_on(async {
        let (host, exports) = super::example_support::start::<Helpers>().await?;
        let key = host.observed().callback("command helpers")?;
        host.observe().actions.value = json!({"fuzzy.project":true,"fuzzy.filter":[99]});
        let result = exports
            .invoke_command(
                key,
                json!({"operation":"f.fuzzyFilter","args":["ab"]}).to_string(),
                host.command_context("/helpers"),
            )
            .await
            .unwrap_err();
        assert_eq!(result, "Native fuzzy selection index 99 is out of range");
        super::example_support::release_all(&host, &["command helpers"]);
        Ok(())
    })
}
