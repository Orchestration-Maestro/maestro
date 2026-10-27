//! Custom component and overlay operations declared once on the shared import interface.

/// Declares custom component and overlay operations without creating a second adapter trait.
macro_rules! overlay_imports {
    () => {
        /// The native accepted-close operation of one custom component.
        type CustomClose: 'static;
        /// Attempts to close, retaining the host's first-close decision.
        fn custom_close(&self, close: &Self::CustomClose) -> ExtensionResult<bool>;
        /// Shows one custom component on the original UI until it closes.
        fn ui_custom<'a>(
            &'a self,
            resource: &'a Self::Ui,
            factory: &'a Self::Callback,
            options: Option<super::custom::CustomOptionsRef<'a, Self>>,
        ) -> ExtensionFuture<'a, ()>;
        /// Retained native overlay placement.
        type OverlayOptions: 'static;
        /// Creates one shared placement owner.
        fn new_overlay_options(&self, encoded: &str) -> ExtensionResult<Self::OverlayOptions>;
        /// Reads current placement from its original owner.
        fn overlay_options_get(&self, options: &Self::OverlayOptions) -> ExtensionResult<String>;
        /// Replaces placement on the original owner.
        fn overlay_options_set(
            &self,
            options: &Self::OverlayOptions,
            encoded: &str,
        ) -> ExtensionResult<()>;
        /// Another owner of the same native placement record.
        fn overlay_options_share(&self, options: &Self::OverlayOptions) -> Self::OverlayOptions;
        /// Replaces or removes the authored visibility check staged under `visible`.
        fn overlay_options_set_visible(
            &self,
            options: &Self::OverlayOptions,
            visible: Option<&Self::Callback>,
        ) -> ExtensionResult<()>;
        /// A shown overlay's handle.
        type OverlayHandle: 'static;
        /// Runs one request on a shown overlay; a flag request answers its flag.
        fn overlay_handle_request(
            &self,
            handle: &Self::OverlayHandle,
            request: super::overlays::OverlayRequest,
        ) -> ExtensionResult<Option<bool>>;
    };
}
