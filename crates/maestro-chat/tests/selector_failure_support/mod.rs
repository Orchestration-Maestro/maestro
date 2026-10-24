//! Strict recorded selector failures and shared original diagnostics.
use maestro_app::OAuthError;
pub use maestro_credentials::test_support::fallback_support::errors;
use serde::Deserialize;
use serde_json::Value;
/// One recorded transaction.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub test: String,
    pub input: Input,
    pub expected: Value,
}
/// Complete caller input domains for the two selectors.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Input {
    pub scoped: Option<bool>,
    pub message: Option<String>,
    pub initial: Option<String>,
    pub width: Option<usize>,
    pub notice: Option<String>,
    pub ops: Option<Vec<String>>,
    pub phase: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    pub supplied: Option<bool>,
    #[cfg(not(target_arch = "wasm32"))]
    pub action: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    pub failure_index: Option<usize>,
    #[cfg(not(target_arch = "wasm32"))]
    pub branch: Option<String>,
}
/// All recorded transactions owned by a named public witness.
pub fn cases(test: &str) -> Vec<Case> {
    let rows: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/fallback_failures.json")).unwrap();
    let rows: Vec<_> = rows.into_iter().filter(|row| row.test == test).collect();
    assert!(!rows.is_empty());
    rows
}
/// Observe a boxed native callback failure without rebuilding it.
#[cfg(not(target_arch = "wasm32"))]
pub fn error_value(error: &(dyn std::error::Error + 'static)) -> Value {
    errors::error_value(
        error
            .downcast_ref::<OAuthError>()
            .expect("original account error"),
    )
    .unwrap()
}
