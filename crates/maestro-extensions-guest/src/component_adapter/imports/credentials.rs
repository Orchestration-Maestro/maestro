//! Credential resources and constructors declared once on the shared import interface.

/// Declares the credential family without creating a second adapter trait.
macro_rules! credential_imports {
    () => {
        /// Independently retained native OAuth implementation.
        type OAuthProvider: 'static;
        /// Lists callable implementations rather than data-only descriptors.
        fn credentials_oauth_providers(
            &self,
            owner: &Self::Credentials,
        ) -> ExtensionResult<Vec<Self::OAuthProvider>>;
        /// Reads the literal identifier, label and optional callback-server metadata.
        fn oauth_info(&self, provider: &Self::OAuthProvider) -> ExtensionResult<String>;
        /// Runs native authorization with retained callbacks and caller cancellation.
        fn oauth_login<'a>(
            &'a self,
            provider: &'a Self::OAuthProvider,
            handler: &'a Self::Callback,
            controls: ((bool, bool, bool), Option<&'a Self::Signal>),
        ) -> ExtensionFuture<'a, String>;
        /// Delegates login and persistence to the retained credential owner.
        fn credentials_login<'a>(
            &'a self,
            owner: &'a Self::Credentials,
            provider: String,
            handler: &'a Self::Callback,
            controls: ((bool, bool, bool), Option<&'a Self::Signal>),
        ) -> ExtensionFuture<'a, ()>;
        /// Awaits the original provider's refresh operation.
        fn oauth_refresh<'a>(
            &'a self,
            provider: &'a Self::OAuthProvider,
            credentials: String,
        ) -> ExtensionFuture<'a, String>;
        /// Reads an optional transformed descriptor list, encoded as JSON.
        fn oauth_models(
            &self,
            provider: &Self::OAuthProvider,
            data: &str,
        ) -> ExtensionResult<String>;
        /// Converts open credentials using the retained provider implementation.
        fn oauth_key(
            &self,
            provider: &Self::OAuthProvider,
            credentials: &str,
        ) -> ExtensionResult<String>;

        /// Invocation-local native lock update, never a serialized resource identity.
        type LockUpdate: 'static;
        /// Calls the actual still-borrowed native update.
        fn lock_update(
            &self,
            update: &Self::LockUpdate,
            current: Option<&str>,
        ) -> ExtensionResult<Option<String>>;
        /// Awaits the actual native update on its original operation's pending set.
        fn lock_update_async<'a>(
            &'a self,
            update: &'a Self::LockUpdate,
            current: Option<String>,
        ) -> ExtensionFuture<'a, Option<String>>;
        /// Constructs the native credential owner from an authored backend.
        fn auth_storage_from_backend(
            &self,
            backend: &Self::Callback,
        ) -> ExtensionResult<Self::Credentials>;
        /// Replaces the native owner's retained authored fallback.
        fn credentials_set_fallback(
            &self,
            owner: &Self::Credentials,
            resolver: &Self::Callback,
        ) -> ExtensionResult<()>;
        /// Creates file credentials; the host resolves absent paths.
        fn create_auth_storage(&self, path: Option<&str>) -> ExtensionResult<Self::Credentials>;
        /// Creates credentials without file access.
        fn auth_storage_in_memory(&self, data: &str) -> ExtensionResult<Self::Credentials>;
        /// Creates a catalog over the original credential resource.
        fn create_model_registry(
            &self,
            auth: &Self::Credentials,
            path: Option<&str>,
        ) -> ExtensionResult<Self::Registry>;
        /// Creates a catalog without file access.
        fn in_memory_model_registry(
            &self,
            auth: &Self::Credentials,
        ) -> ExtensionResult<Self::Registry>;
        /// Independently retained native credentials.
        type Credentials: 'static;
        /// Returns the original catalog's credential owner.
        fn registry_auth_storage(
            &self,
            resource: &Self::Registry,
        ) -> ExtensionResult<Self::Credentials>;

        /// Forwards the corresponding credential operation.
        fn credentials_set_runtime_api_key(
            &self,
            resource: &Self::Credentials,
            provider: &str,
            key: &str,
        ) -> ExtensionResult<()>;
        /// Forwards the corresponding credential operation.
        fn credentials_remove_runtime_api_key(
            &self,
            resource: &Self::Credentials,
            provider: &str,
        ) -> ExtensionResult<()>;
        /// Forwards the corresponding credential operation.
        fn credentials_reload(&self, resource: &Self::Credentials) -> ExtensionResult<()>;
        /// Forwards the corresponding credential operation.
        fn credentials_get(
            &self,
            resource: &Self::Credentials,
            provider: &str,
        ) -> ExtensionResult<Option<String>>;
        /// Forwards the corresponding credential operation.
        fn credentials_set(
            &self,
            resource: &Self::Credentials,
            provider: &str,
            credential: &str,
        ) -> ExtensionResult<()>;
        /// Forwards the corresponding credential operation.
        fn credentials_remove(
            &self,
            resource: &Self::Credentials,
            provider: &str,
        ) -> ExtensionResult<()>;
        /// Forwards the corresponding credential operation.
        fn credentials_list(&self, resource: &Self::Credentials) -> ExtensionResult<Vec<String>>;
        /// Forwards the corresponding credential operation.
        fn credentials_has(
            &self,
            resource: &Self::Credentials,
            provider: &str,
        ) -> ExtensionResult<bool>;
        /// Forwards the corresponding credential operation.
        fn credentials_has_auth(
            &self,
            resource: &Self::Credentials,
            provider: &str,
        ) -> ExtensionResult<bool>;
        /// Forwards the corresponding credential operation.
        fn credentials_get_auth_status(
            &self,
            resource: &Self::Credentials,
            provider: &str,
        ) -> ExtensionResult<String>;
        /// Forwards the corresponding credential operation.
        fn credentials_get_all(&self, resource: &Self::Credentials) -> ExtensionResult<String>;
        /// Forwards the corresponding credential operation.
        fn credentials_drain_errors(&self, resource: &Self::Credentials)
        -> ExtensionResult<String>;
        /// Forwards the corresponding credential operation.
        fn credentials_logout(
            &self,
            resource: &Self::Credentials,
            provider: &str,
        ) -> ExtensionResult<()>;
        /// Resolves a key through the retained native credential effects.
        fn credentials_get_api_key<'a>(
            &'a self,
            resource: &'a Self::Credentials,
            provider: String,
            include_fallback: Option<bool>,
        ) -> ExtensionFuture<'a, Option<String>>;
    };
}
