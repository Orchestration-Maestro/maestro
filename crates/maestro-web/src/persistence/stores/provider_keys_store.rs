//! The provider-keys collection.
use super::super::{Store, StoreConfig};
/// Typed access to the provider-keys collection.
pub struct ProviderKeysStore(Store);
impl Default for ProviderKeysStore {
    fn default() -> Self {
        Self(Store::new(StoreConfig {
            name: "provider-keys".into(),
            key_path: None,
            auto_increment: None,
            indices: None,
        }))
    }
}
impl std::ops::Deref for ProviderKeysStore {
    type Target = Store;
    fn deref(&self) -> &Store {
        &self.0
    }
}
impl ProviderKeysStore {
    /// Read a provider's key, retaining empty strings.
    ///
    /// # Errors
    /// Returns binding or adapter failures, or `InvalidData` on a serde conversion failure.
    pub async fn get(&self, provider: &str) -> std::io::Result<Option<String>> {
        self.0.read(provider).await
    }
    /// Replace a provider's key without normalization.
    ///
    /// # Errors
    /// Returns binding or adapter failures, or `InvalidData` on a serde conversion failure.
    pub async fn set(&self, provider: &str, key: &str) -> std::io::Result<()> {
        self.0.write(provider, key).await
    }
    /// Check key presence independently of its contents.
    ///
    /// # Errors
    /// Returns binding or adapter failures.
    pub async fn has(&self, provider: &str) -> std::io::Result<bool> {
        self.get_backend()?.has("provider-keys", provider).await
    }

    /// Delete a provider's key, including an absent key.
    ///
    /// # Errors
    /// Returns binding or adapter failures.
    pub async fn delete(&self, provider: &str) -> std::io::Result<()> {
        self.get_backend()?.delete("provider-keys", provider).await
    }
    /// Enumerate provider identifiers in adapter order.
    ///
    /// # Errors
    /// Returns binding or adapter failures.
    pub async fn list(&self) -> std::io::Result<Vec<String>> {
        self.get_backend()?.keys("provider-keys", None).await
    }
}
