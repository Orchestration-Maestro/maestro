//! Model, message and content records.
pub use crate::bindings::maestro::extension::models::{
    AnthropicMessagesCompat, Api, AssistantBlock, AssistantMessage, AssistantMessageEvent,
    CacheControlFormat, ContentBlock, ContentDelta, ContentEnd, ContentStart, Cost, DataCollection,
    DoneEvent, DoneReason, ErrorEvent, ErrorReason, ImageContent, InputKind, LevelValue, MaxPrice,
    MaxTokensField, Model, ModelCompat, ModelCost, NullableString, OpenRouterRouting,
    OpenaiCompletionsCompat as OpenAICompletionsCompat,
    OpenaiResponsesCompat as OpenAIResponsesCompat, PercentilePreference, Percentiles, Price,
    Provider, SortDetail, SortPreference, StopReason, TextContent, ThinkingContent, ThinkingFormat,
    ThinkingLevelMap, ToolCall, ToolCallEnd, ToolResultMessage, Usage, UserContent, UserMessage,
    VercelGatewayRouting,
};
