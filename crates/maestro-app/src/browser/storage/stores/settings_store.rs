//! The settings collection.
use super::super::{Store, StoreConfig};
/// Typed access to the settings collection.
pub struct SettingsStore(Store);
impl Default for SettingsStore {
    fn default() -> Self {
        Self(Store::new(StoreConfig {
            name: "settings".into(),
            key_path: None,
            auto_increment: None,
            indices: None,
        }))
    }
}
impl std::ops::Deref for SettingsStore {
    type Target = Store;
    fn deref(&self) -> &Store {
        &self.0
    }
}
impl SettingsStore {
    /// Read a typed setting; absent or null yields no value.
    ///
    /// # Errors
    /// Returns binding or adapter failures, or `InvalidData` on a serde conversion failure.
    pub async fn get<T: serde::de::DeserializeOwned>(
        &self,
        key: &str,
    ) -> std::io::Result<Option<T>> {
        self.0.read(key).await
    }

    /// Write a serializable setting without changing its key.
    ///
    /// # Errors
    /// Returns binding or adapter failures, or `InvalidData` on a serde conversion failure.
    pub async fn set<T: serde::Serialize>(&self, key: &str, value: &T) -> std::io::Result<()> {
        self.0.write(key, value).await
    }

    /// Delete a setting, including an absent key.
    ///
    /// # Errors
    /// Returns binding or adapter failures.
    pub async fn delete(&self, key: &str) -> std::io::Result<()> {
        self.get_backend()?.delete("settings", key).await
    }
    /// Enumerate setting keys in adapter order.
    ///
    /// # Errors
    /// Returns binding or adapter failures.
    pub async fn list(&self) -> std::io::Result<Vec<String>> {
        self.get_backend()?.keys("settings", None).await
    }
    /// Clear only the settings collection.
    ///
    /// # Errors
    /// Returns binding or adapter failures.
    pub async fn clear(&self) -> std::io::Result<()> {
        self.get_backend()?.clear("settings").await
    }
}
