//! Tool kinds, function declarations and tool configuration of the selected endpoint.

use serde_json::{Map, Value};
use std::mem;

use super::{Endpoint, Failure, Projection, Step, record, schema};
use maestro_json::is_truthy;

/// Selection of a tool, in wire order; one tool may hold several kinds.
const TOOL: &[Step] = &[
    Step::Reject("retrieval"),
    Step::Keep("computerUse"),
    Step::Keep("fileSearch"),
    Step::Convert("googleSearch", |search, endpoint| {
        record(search, SEARCH, endpoint)
    }),
    Step::Convert("googleMaps", |maps, endpoint| record(maps, MAPS, endpoint)),
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
    Step::Convert("authConfig", |auth, endpoint| record(auth, AUTH, endpoint)),
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
    Step::Convert("functionCallingConfig", |calling, endpoint| {
        record(calling, FUNCTION_CALLING, endpoint)
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
pub(super) fn tools(list: Value, endpoint: Endpoint<'_>) -> Projection<Value> {
    let Value::Array(items) = list else {
        return Err(Failure::error(
            "tools is required and must be an array of Tools",
        ));
    };
    items
        .into_iter()
        .map(|tool| {
            let plan = match endpoint {
                Endpoint::Public => TOOL,
                Endpoint::Vertex { .. } => VERTEX_TOOL,
            };
            record(prepare(tool)?, plan, endpoint)
        })
        .collect::<Projection<_>>()
        .map(Value::Array)
}

/// The wire form of a tool configuration.
pub(super) fn tool_config(config: Value, endpoint: Endpoint<'_>) -> Projection<Value> {
    let plan = match endpoint {
        Endpoint::Public => TOOL_CONFIG,
        Endpoint::Vertex { .. } => VERTEX_TOOL_CONFIG,
    };
    record(config, plan, endpoint)
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

/// Convert a truthy structured declaration schema, or apply the schema owner's routing rule.
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
    if schema::is_json_schema(current) {
        schema::route_json_schema(declaration, schema_key, json_key);
    } else {
        *current = schema::convert(mem::take(current))?;
    }
    Ok(())
}

/// Cloud tool selection after shared schema preparation.
const VERTEX_TOOL: &[Step] = &[
    Step::Keep("retrieval"),
    Step::Keep("computerUse"),
    Step::Reject("fileSearch"),
    Step::Keep("googleSearch"),
    Step::Keep("googleMaps"),
    Step::Keep("codeExecution"),
    Step::Keep("enterpriseWebSearch"),
    Step::Convert("functionDeclarations", |value, endpoint| {
        super::record_list(value, VERTEX_DECLARATION, endpoint)
    }),
    Step::Keep("googleSearchRetrieval"),
    Step::Keep("parallelAiSearch"),
    Step::Keep("urlContext"),
    Step::Reject("mcpServers"),
];

/// Cloud declaration fields after the whole-list schema pass.
const VERTEX_DECLARATION: &[Step] = &[
    Step::Keep("description"),
    Step::Keep("name"),
    Step::Keep("parameters"),
    Step::Keep("parametersJsonSchema"),
    Step::Keep("response"),
    Step::Keep("responseJsonSchema"),
    Step::Reject("behavior"),
];

/// Cloud tool configuration retains the complete calling record.
const VERTEX_TOOL_CONFIG: &[Step] = &[
    Step::Keep("retrievalConfig"),
    Step::Keep("functionCallingConfig"),
    Step::Reject("includeServerSideToolInvocations"),
];
