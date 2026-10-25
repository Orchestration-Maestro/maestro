//! Browser contribution records accepted from packages.
use serde::{Deserialize, Serialize};

/// One contribution as authored in a package's contribution file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContributionDescriptor {
    /// Identifier, unique within its package.
    pub id: String,
    /// Extensible kind, such as `tool-renderer`, `message-renderer`, `page`, `card` or `api`.
    pub kind: String,
    /// Tool name, message role, slot name or API name, selected by the kind.
    pub target: String,
    /// Route reference of a routable contribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    /// Consumed data contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    /// Child slots supplied to other contributions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<ContributionSlot>,
    /// Required host API names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
    /// Separately built browser module that registers this contribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<BrowserModule>,
    /// Package-relative document shown in an isolated frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<String>,
}
/// One named child slot and the data contract it passes on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ContributionSlot {
    /// Slot name.
    pub name: String,
    /// Data contract passed to attached contributions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
}
/// Package-relative assets of one separately built browser module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BrowserModule {
    /// Module entry generated beside the WebAssembly binary.
    pub entry: String,
    /// Matching WebAssembly binary.
    pub wasm: String,
    /// Exported registration function.
    pub register: String,
}
/// One enabled contribution with the package that supplied it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BrowserContribution {
    /// Supplying package source.
    pub package: String,
    /// Authored declaration.
    pub descriptor: ContributionDescriptor,
}
