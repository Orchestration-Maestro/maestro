//! The custom-providers collection.
use super::super::{Store, StoreConfig};
/// Typed access to the custom-providers collection.
pub struct CustomProvidersStore(Store);
impl Default for CustomProvidersStore {
    fn default() -> Self {
        Self(Store::new(StoreConfig {
            name: "custom-providers".into(),
            key_path: None,
            auto_increment: None,
            indices: None,
        }))
    }
}
impl std::ops::Deref for CustomProvidersStore {
    type Target = Store;
    fn deref(&self) -> &Store {
        &self.0
    }
}

use crate::{Model, Presence};
use serde::{Deserialize, Serialize};
/// Provider families supporting model discovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutoDiscoveryProviderType {
    /// Ollama server.
    #[serde(rename = "ollama")]
    Ollama,
    /// llama.cpp server.
    #[serde(rename = "llama.cpp")]
    LlamaCpp,
    /// vLLM server.
    #[serde(rename = "vllm")]
    Vllm,
    /// LM Studio server.
    #[serde(rename = "lmstudio")]
    Lmstudio,
}
/// Protocols for manually supplied models.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManualProviderType {
    /// Chat completions protocol.
    OpenaiCompletions,
    /// Responses protocol.
    OpenaiResponses,
    /// Messages protocol.
    AnthropicMessages,
}
/// Discovery family or manual model protocol, encoded as one literal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CustomProviderType {
    /// Discoverable server family.
    AutoDiscovery(AutoDiscoveryProviderType),
    /// Manually configured protocol.
    Manual(ManualProviderType),
}
/// Persisted custom provider descriptor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomProvider {
    /// Provider identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Server family or model protocol.
    pub r#type: CustomProviderType,
    /// Server base URL.
    pub base_url: String,
    /// Optional key, distinguishing omission from null.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub api_key: Presence<String>,
    /// Optional complete model descriptors.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub models: Presence<Vec<Model>>,
}
impl CustomProvidersStore {
    /// Read a provider by identifier.
    ///
    /// # Errors
    /// Returns binding or adapter failures, or `InvalidData` on a serde conversion failure.
    pub async fn get(&self, id: &str) -> std::io::Result<Option<CustomProvider>> {
        self.0.read(id).await
    }
    /// Replace the descriptor at its identifier.
    ///
    /// # Errors
    /// Returns binding or adapter failures, or `InvalidData` on a serde conversion failure.
    pub async fn set(&self, provider: &CustomProvider) -> std::io::Result<()> {
        self.0.write(&provider.id, provider).await
    }
    /// Check descriptor presence.
    ///
    /// # Errors
    /// Returns binding or adapter failures.
    pub async fn has(&self, id: &str) -> std::io::Result<bool> {
        self.get_backend()?.has("custom-providers", id).await
    }

    /// Delete a descriptor, including an absent identifier.
    ///
    /// # Errors
    /// Returns binding or adapter failures.
    pub async fn delete(&self, id: &str) -> std::io::Result<()> {
        self.get_backend()?.delete("custom-providers", id).await
    }

    /// Read sequentially in key order, skipping missing rows and stopping on failure.
    ///
    /// # Errors
    /// Returns binding or adapter failures, or `InvalidData` on a serde conversion failure.
    pub async fn get_all(&self) -> std::io::Result<Vec<CustomProvider>> {
        let keys = self.get_backend()?.keys("custom-providers", None).await?;
        let mut providers = Vec::new();
        for key in keys {
            if let Some(provider) = self.get(&key).await? {
                providers.push(provider);
            }
        }
        Ok(providers)
    }
}
