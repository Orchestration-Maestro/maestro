//! Regular expressions a schema builds before any assertion is evaluated.

use std::collections::{HashMap, hash_map::Entry};

use serde_json::{Map, Value};

use super::{
    check::{is_schema, schema_array, schema_map},
    collections::dependency_map,
    diagnostics,
    references::{self, Context, Location},
};

/// Unicode expressions of one schema, keyed by their pattern source.
pub(super) struct Patterns<'a>(HashMap<&'a str, regress::Regex>);

impl Patterns<'_> {
    /// The expression built for a pattern source that a reachable schema declares.
    pub fn get(&self, source: &str) -> Option<&regress::Regex> {
        self.0.get(source)
    }
}

/// Work remaining in the walk, in the order the schema's keywords build.
enum Step<'a> {
    /// Build the expression of one pattern source.
    Pattern(&'a str),
    /// Build a schema nested in the one being built.
    Schema(Context<'a>),
    /// Build a root or reference target, unless that context was built already.
    Target(Context<'a>),
}

/// Build every expression of the schemas that checking can reach, or return the first
/// failure in build order.
///
/// A schema is reached through its keywords' nested schemas and through reference targets,
/// whatever the arguments are; unreferenced definitions and other members are never built.
/// A target is expanded once for each set of live anchor bindings that reaches it, because a
/// dynamic reference selects its target from those bindings. Each pattern source is built once
/// and shared by every schema that declares it.
pub(super) fn prepare<'a>(root: &Location<'a>) -> Result<Patterns<'a>, regress::Error> {
    let mut patterns = HashMap::new();
    let mut built: Vec<Context<'a>> = Vec::new();
    let mut steps = vec![Step::Target(Context::root(root.clone()))];
    while let Some(step) = steps.pop() {
        match step {
            Step::Pattern(source) => {
                if let Entry::Vacant(slot) = patterns.entry(source) {
                    slot.insert(regress::Regex::with_flags(source, "u")?);
                }
            }
            Step::Target(context) => {
                if built.iter().all(|built| !built.same(&context)) {
                    steps.extend(expand(root, &context).into_iter().rev());
                    built.push(context);
                }
            }
            Step::Schema(context) => steps.extend(expand(root, &context).into_iter().rev()),
        }
    }
    Ok(Patterns(patterns))
}

/// Entries of an optional map in the order checking visits them.
fn entries(map: Option<&Map<String, Value>>) -> Vec<(&String, &Value)> {
    map.map_or_else(Vec::new, diagnostics::entries)
}

/// Steps building one schema, collected while its keywords are read in build order.
struct Expansion<'c, 'a> {
    /// Context of the schema being built; nested schemas descend from it.
    context: &'c Context<'a>,
    /// Schema being built.
    schema: &'a Value,
    /// Steps in build order.
    steps: Vec<Step<'a>>,
}

impl<'a> Expansion<'_, 'a> {
    /// Build nested schemas in order.
    fn schemas(&mut self, schemas: impl IntoIterator<Item = &'a Value>) {
        for schema in schemas {
            self.steps.push(Step::Schema(self.context.child(schema)));
        }
    }

    /// Build the schema a keyword holds, when it holds one.
    fn keyword(&mut self, keyword: &str) {
        self.schemas(self.schema.get(keyword).filter(|nested| is_schema(nested)));
    }

    /// Build each schema of a keyword's list, when every member is a schema.
    fn list(&mut self, keyword: &str) {
        self.schemas(
            self.schema
                .get(keyword)
                .and_then(schema_array)
                .into_iter()
                .flatten(),
        );
    }

    /// Build the object keywords: member schemas and the expressions of pattern keys.
    fn object(&mut self) {
        let patterns = entries(schema_map(self.schema.get("patternProperties")));
        if self
            .schema
            .get("additionalProperties")
            .is_some_and(is_schema)
        {
            self.steps
                .extend(patterns.iter().map(|(source, _)| Step::Pattern(source)));
            self.keyword("additionalProperties");
        }
        for keyword in ["dependencies", "dependentSchemas"] {
            let dependencies = entries(dependency_map(self.schema, keyword));
            self.schemas(
                dependencies
                    .into_iter()
                    .map(|(_, nested)| nested)
                    .filter(|nested| is_schema(nested)),
            );
        }
        for (source, nested) in patterns {
            self.steps.push(Step::Pattern(source));
            self.schemas([nested]);
        }
        let properties = entries(schema_map(self.schema.get("properties")));
        self.schemas(properties.into_iter().map(|(_, nested)| nested));
        self.keyword("propertyNames");
    }

    /// Build the array keywords; `contains` follows `items` when `minContains` is zero.
    fn array(&mut self) {
        let tuple = self.schema.get("items").and_then(schema_array);
        if tuple.is_some() {
            self.keyword("additionalItems");
        }
        let zero_minimum = self.schema.get("minContains").and_then(Value::as_f64) == Some(0.0);
        if !zero_minimum {
            self.keyword("contains");
        }
        self.keyword("items");
        self.schemas(tuple.into_iter().flatten());
        if zero_minimum {
            self.keyword("contains");
        }
        self.list("prefixItems");
    }

    /// Build the conditional, negation, combinator and unevaluated keywords.
    fn composition(&mut self) {
        if self.schema.get("if").is_some_and(is_schema) {
            for keyword in ["if", "then", "else"] {
                self.keyword(keyword);
            }
        }
        self.keyword("not");
        for keyword in ["allOf", "anyOf", "oneOf"] {
            self.list(keyword);
        }
        for keyword in ["unevaluatedItems", "unevaluatedProperties"] {
            self.keyword(keyword);
        }
    }
}

/// Steps building the schema a context applies, in the order its keywords build.
fn expand<'a>(root: &Location<'a>, context: &Context<'a>) -> Vec<Step<'a>> {
    let mut build = Expansion {
        context,
        schema: context.location.schema,
        steps: Vec::new(),
    };
    build.object();
    build.array();
    if let Some(source) = build.schema.get("pattern").and_then(Value::as_str) {
        build.steps.push(Step::Pattern(source));
    }
    build.steps.extend(
        references::targets(root, context)
            .into_iter()
            .flatten()
            .map(|target| Step::Target(context.follow(target))),
    );
    build.composition();
    build.steps
}
