//! Tool kinds, function declarations and tool configuration of the public endpoint.

use serde_json::{Map, Value};
use std::mem;

use super::{Failure, Projection, Step, malformed, record, schema};
use maestro_json::is_truthy;

/// Selection of a tool, in wire order; one tool may hold several kinds.
const TOOL: &[Step] = &[
    Step::Reject("retrieval"),
    Step::Keep("computerUse"),
    Step::Keep("fileSearch"),
    Step::Convert("googleSearch", |search| record(search, SEARCH)),
    Step::Convert("googleMaps", |maps| record(maps, MAPS)),
    Step::Keep("codeExecution"),
    Step::Reject("enterpriseWebSearch"),
    Step::Keep("functionDeclarations"),
    Step::Keep("googleSearchRetrieval"),
    Step::Reject("parallelAiSearch"),
    Step::Keep("urlContext"),
    Step::Keep("mcpServers"),
];

/// Selection of the search tool.
const SEARCH: &[Step] = &[
    Step::Keep("searchTypes"),
    Step::Reject("blockingConfidence"),
    Step::Reject("excludeDomains"),
    Step::Keep("timeRangeFilter"),
];

/// Selection of the maps tool.
const MAPS: &[Step] = &[
    Step::Convert("authConfig", |auth| record(auth, AUTH)),
    Step::Keep("enableWidget"),
];

/// Selection of the maps credential record.
const AUTH: &[Step] = &[
    Step::Keep("apiKey"),
    Step::Reject("apiKeyConfig"),
    Step::Reject("authType"),
    Step::Reject("googleServiceAccountConfig"),
    Step::Reject("httpBasicAuthConfig"),
    Step::Reject("oauthConfig"),
    Step::Reject("oidcConfig"),
];

/// Selection of the tool configuration.
const TOOL_CONFIG: &[Step] = &[
    Step::Keep("retrievalConfig"),
    Step::Convert("functionCallingConfig", |calling| {
        record(calling, FUNCTION_CALLING)
    }),
    Step::Keep("includeServerSideToolInvocations"),
];

/// Selection of the function-calling configuration.
const FUNCTION_CALLING: &[Step] = &[
    Step::Keep("allowedFunctionNames"),
    Step::Keep("mode"),
    Step::Reject("streamFunctionCallArguments"),
];

/// The wire form of a tool list; each tool's schemas are prepared before its members are
/// selected.
pub(super) fn tools(list: Value) -> Projection<Value> {
    let Value::Array(items) = list else {
        return Err(Failure::error(
            "tools is required and must be an array of Tools",
        ));
    };
    items
        .into_iter()
        .map(|tool| record(prepare(tool)?, TOOL))
        .collect::<Projection<_>>()
        .map(Value::Array)
}

/// The wire form of a tool configuration.
pub(super) fn tool_config(config: Value) -> Projection<Value> {
    record(config, TOOL_CONFIG)
}

/// Convert or reroute the schemas of every function declaration of a tool.
fn prepare(mut tool: Value) -> Projection<Value> {
    if let Some(Value::Array(declarations)) = tool.get_mut("functionDeclarations") {
        for declaration in declarations.iter_mut().filter_map(Value::as_object_mut) {
            reroute(declaration, "parameters", "parametersJsonSchema")?;
            reroute(declaration, "response", "responseJsonSchema")?;
        }
    }
    Ok(tool)
}

/// Convert a structured schema in place, or move a JSON schema to its alternative member
/// unless that member is already populated.
fn reroute(
    declaration: &mut Map<String, Value>,
    schema_key: &str,
    json_key: &str,
) -> Projection<()> {
    let Some(current) = declaration
        .get_mut(schema_key)
        .filter(|value| is_truthy(value))
    else {
        return Ok(());
    };
    if !schema::is_json_schema(current) {
        *current = schema::convert(mem::take(current))?;
    } else if !declaration.get(json_key).is_some_and(is_truthy) {
        let moved = declaration
            .shift_remove(schema_key)
            .ok_or_else(|| malformed("schema vanished"))?;
        declaration.insert(json_key.to_owned(), moved);
    }
    Ok(())
}
