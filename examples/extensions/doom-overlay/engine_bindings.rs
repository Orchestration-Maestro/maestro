//! Compile-time bindings for the example's engine interface.
/// Generates imports or exports from the one engine interface source.
macro_rules! generate {
    ($world:literal) => {
        /// Generated engine bindings.
        #[allow(clippy::same_length_and_capacity)]
        mod bindings {
            wit_bindgen::generate!({
                path: "../../examples/extensions/doom-overlay/engine.wit",
                world: $world,
                pub_export_macro: true,
                default_bindings_module: "bindings",
            });
        }
    };
}
pub(crate) use generate;
