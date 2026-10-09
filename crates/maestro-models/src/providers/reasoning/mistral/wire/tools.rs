//! Typed tools transport records.

use super::{Nullable, Object, literal, nullable, optional};
use serde::{Deserialize, Serialize};

/// Transport fields for `Tool`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct Tool {
    /// The `type` field.
    #[serde(rename = "type")]
    #[serde(deserialize_with = "literal")]
    kind: ToolTag,
    /// The `function` field.
    function: Object<FunctionT>,
}

/// Transport fields for `FunctionT`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct FunctionT {
    /// The `name` field.
    name: String,
    /// The `description` field.
    #[serde(
        default,
        deserialize_with = "optional",
        skip_serializing_if = "Option::is_none"
    )]
    description: Option<String>,
    /// The `strict` field.
    #[serde(
        default,
        deserialize_with = "optional",
        skip_serializing_if = "Option::is_none"
    )]
    strict: Option<bool>,
    /// The `parameters` field.
    parameters: serde_json::Map<String, serde_json::Value>,
}

/// Transport fields for `ToolChoice`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct ToolChoice {
    /// The `type` field.
    #[serde(
        default,
        deserialize_with = "optional",
        skip_serializing_if = "Option::is_none",
        rename = "type"
    )]
    kind: Option<String>,
    /// The `function` field.
    function: Object<FunctionName>,
}

/// Transport fields for `FunctionName`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct FunctionName {
    /// The `name` field.
    name: String,
}

/// Transport fields for `ToolConfiguration`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct ToolConfiguration {
    /// The `exclude` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    exclude: Option<Nullable<Vec<String>>>,
    /// The `include` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    include: Option<Nullable<Vec<String>>>,
    /// The `requiresConfirmation` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none",
        rename(
            deserialize = "requiresConfirmation",
            serialize = "requires_confirmation"
        )
    )]
    requires_confirmation: Option<Nullable<Vec<String>>>,
}

/// Transport fields for `CustomConnector`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct CustomConnector {
    /// The `type` field.
    #[serde(rename = "type")]
    #[serde(deserialize_with = "literal")]
    kind: CustomConnectorTag,
    /// The `connectorId` field.
    connector_id: String,
    /// The `authorization` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    authorization: Option<Nullable<Authorization>>,
    /// The `toolConfiguration` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    tool_configuration: Option<Nullable<Object<ToolConfiguration>>>,
}

/// Transport fields for `APIKeyAuth`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct APIKeyAuth {
    /// The `type` field.
    #[serde(rename = "type")]
    #[serde(deserialize_with = "literal")]
    kind: APIKeyAuthTag,
    /// The `value` field.
    value: String,
}

/// Transport fields for `OAuth2TokenAuth`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct OAuth2TokenAuth {
    /// The `type` field.
    #[serde(rename = "type")]
    #[serde(deserialize_with = "literal")]
    kind: OAuth2TokenAuthTag,
    /// The `value` field.
    value: String,
}

/// Transport fields for `WebSearchTool`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct WebSearchTool {
    /// The `toolConfiguration` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    tool_configuration: Option<Nullable<Object<ToolConfiguration>>>,
    /// The `type` field.
    #[serde(rename = "type")]
    #[serde(deserialize_with = "literal")]
    kind: WebSearchToolTag,
}

/// Transport fields for `WebSearchPremiumTool`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct WebSearchPremiumTool {
    /// The `toolConfiguration` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    tool_configuration: Option<Nullable<Object<ToolConfiguration>>>,
    /// The `type` field.
    #[serde(rename = "type")]
    #[serde(deserialize_with = "literal")]
    kind: WebSearchPremiumToolTag,
}

/// Transport fields for `CodeInterpreterTool`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct CodeInterpreterTool {
    /// The `toolConfiguration` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    tool_configuration: Option<Nullable<Object<ToolConfiguration>>>,
    /// The `type` field.
    #[serde(rename = "type")]
    #[serde(deserialize_with = "literal")]
    kind: CodeInterpreterToolTag,
}

/// Transport fields for `ImageGenerationTool`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct ImageGenerationTool {
    /// The `toolConfiguration` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    tool_configuration: Option<Nullable<Object<ToolConfiguration>>>,
    /// The `type` field.
    #[serde(rename = "type")]
    #[serde(deserialize_with = "literal")]
    kind: ImageGenerationToolTag,
}

/// Transport fields for `DocumentLibraryTool`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct DocumentLibraryTool {
    /// The `toolConfiguration` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    tool_configuration: Option<Nullable<Object<ToolConfiguration>>>,
    /// The `type` field.
    #[serde(rename = "type")]
    #[serde(deserialize_with = "literal")]
    kind: DocumentLibraryToolTag,
    /// The `libraryIds` field.
    library_ids: Vec<String>,
}

/// Literal `function` tag.
#[derive(Deserialize, Serialize)]
pub(super) enum ToolTag {
    /// Transport discriminator.
    #[serde(rename = "function")]
    Value,
}

/// Literal `connector` tag.
#[derive(Deserialize, Serialize)]
pub(super) enum CustomConnectorTag {
    /// Transport discriminator.
    #[serde(rename = "connector")]
    Value,
}

/// Literal `api-key` tag.
#[derive(Deserialize, Serialize)]
pub(super) enum APIKeyAuthTag {
    /// Transport discriminator.
    #[serde(rename = "api-key")]
    Value,
}

/// Literal `oauth2-token` tag.
#[derive(Deserialize, Serialize)]
pub(super) enum OAuth2TokenAuthTag {
    /// Transport discriminator.
    #[serde(rename = "oauth2-token")]
    Value,
}

/// Literal `web_search` tag.
#[derive(Deserialize, Serialize)]
pub(super) enum WebSearchToolTag {
    /// Transport discriminator.
    #[serde(rename = "web_search")]
    Value,
}

/// Literal `web_search_premium` tag.
#[derive(Deserialize, Serialize)]
pub(super) enum WebSearchPremiumToolTag {
    /// Transport discriminator.
    #[serde(rename = "web_search_premium")]
    Value,
}

/// Literal `code_interpreter` tag.
#[derive(Deserialize, Serialize)]
pub(super) enum CodeInterpreterToolTag {
    /// Transport discriminator.
    #[serde(rename = "code_interpreter")]
    Value,
}

/// Literal `image_generation` tag.
#[derive(Deserialize, Serialize)]
pub(super) enum ImageGenerationToolTag {
    /// Transport discriminator.
    #[serde(rename = "image_generation")]
    Value,
}

/// Literal `document_library` tag.
#[derive(Deserialize, Serialize)]
pub(super) enum DocumentLibraryToolTag {
    /// Transport discriminator.
    #[serde(rename = "document_library")]
    Value,
}

/// Tool declarations accepted by the request wire.
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub(super) enum RequestTool {
    /// Function declaration.
    Function(Object<Tool>),
    /// Web search.
    WebSearch(Object<WebSearchTool>),
    /// Premium search.
    WebSearchPremium(Object<WebSearchPremiumTool>),
    /// Code interpreter.
    CodeInterpreter(Object<CodeInterpreterTool>),
    /// Image generation.
    ImageGeneration(Object<ImageGenerationTool>),
    /// Document library.
    DocumentLibrary(Object<DocumentLibraryTool>),
    /// Custom connector.
    Connector(Object<CustomConnector>),
}

/// Open string choice or named function selection.
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub(super) enum ToolSelection {
    /// Named function.
    Function(Object<ToolChoice>),
    /// Open choice string.
    String(String),
}

/// Connector authorization.
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum Authorization {
    /// API key.
    ApiKey(Object<APIKeyAuth>),
    /// OAuth token.
    OAuth(Object<OAuth2TokenAuth>),
}
