//! Catalog operations declared once on the shared import interface.

/// Declares catalog operations without creating a second adapter trait.
macro_rules! catalog_imports {
    () => {
        /// Registers JSON provider data.
        fn register_provider(
            &self,
            name: &str,
            config: &str,
            callbacks: ProviderRegistration<'_, Self::Callback>,
        ) -> ExtensionResult<()>;
        /// Unregisters a provider.
        fn unregister_provider(&self, name: &str) -> ExtensionResult<()>;

        /// Forwards the corresponding host capability call.
        fn registry_get_all(&self, resource: &Self::Registry) -> ExtensionResult<String>;
        /// Forwards the corresponding host capability call.
        fn registry_get_available(&self, resource: &Self::Registry) -> ExtensionResult<String>;
        /// Forwards the corresponding host capability call.
        fn registry_find(
            &self,
            resource: &Self::Registry,
            provider: &str,
            model_id: &str,
        ) -> ExtensionResult<Option<String>>;
        /// Forwards the corresponding host capability call.
        fn registry_get_api_key_and_headers<'a>(
            &'a self,
            resource: &'a Self::Registry,
            model: String,
        ) -> ExtensionFuture<'a, crate::ResolvedRequestAuth>;
        /// Forwards the corresponding host capability call.
        fn registry_refresh(&self, resource: &Self::Registry) -> ExtensionResult<()>;
        /// Forwards the corresponding host capability call.
        fn registry_get_error(&self, resource: &Self::Registry) -> ExtensionResult<Option<String>>;
        /// Forwards the corresponding host capability call with a JSON model.
        fn registry_has_configured_auth(
            &self,
            resource: &Self::Registry,
            model: &str,
        ) -> ExtensionResult<bool>;
        /// Forwards the corresponding host capability call; the status is JSON.
        fn registry_get_provider_auth_status(
            &self,
            resource: &Self::Registry,
            provider: &str,
        ) -> ExtensionResult<String>;
        /// Forwards the corresponding host capability call.
        fn registry_get_provider_display_name(
            &self,
            resource: &Self::Registry,
            provider: &str,
        ) -> ExtensionResult<String>;
        /// Forwards the corresponding host capability call with a JSON model.
        fn registry_is_using_oauth(
            &self,
            resource: &Self::Registry,
            model: &str,
        ) -> ExtensionResult<bool>;
        /// Forwards the corresponding host capability call with JSON provider data.
        fn registry_register_provider(
            &self,
            resource: &Self::Registry,
            name: &str,
            config: &str,
            callbacks: ProviderRegistration<'_, Self::Callback>,
        ) -> ExtensionResult<()>;
        /// Forwards the corresponding host capability call.
        fn registry_unregister_provider(
            &self,
            resource: &Self::Registry,
            name: &str,
        ) -> ExtensionResult<()>;
        /// Forwards the corresponding host capability call.
        fn registry_get_api_key_for_provider<'a>(
            &'a self,
            resource: &'a Self::Registry,
            provider: String,
        ) -> ExtensionFuture<'a, Option<String>>;
    };
}
