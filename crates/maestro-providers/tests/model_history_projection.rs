//! Whole-root replay reaches each protocol adapter.
#[cfg(not(target_arch = "wasm32"))]
#[path = "model_history_projection/root_routes.rs"]
mod root_routes;
#[cfg(test)]
mod tests {
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn history_projections_keep_root_specific_defaults() -> crate::chat::TestResult {
        crate::runtime::run(super::root_routes::check())
    }
}
