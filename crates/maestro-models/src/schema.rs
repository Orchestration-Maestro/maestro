//! Offline checker integration retaining enclosing dialect and reference scope.

use crate::ToolValidationError;
use serde_json::{Value, json};

const ROOT_URI: &str = "json-schema:///tool-parameters";

pub(crate) struct Schema<'a> {
    root: jsonschema::Validator,
    resources: jsonschema::Registry<'a>,
    resource_uri: jsonschema::Uri<String>,
}

impl<'a> Schema<'a> {
    pub(crate) fn build(schema: &'a Value) -> Result<Self, ToolValidationError> {
        let root = jsonschema::options()
            .offline()
            .with_base_uri(ROOT_URI)
            .build(schema)
            .map_err(|_| ToolValidationError::InvalidSchema)?;
        let resources = jsonschema::Registry::new()
            .draft(root.draft())
            .add(ROOT_URI, schema)
            .and_then(|registry| registry.prepare())
            .map_err(|_| ToolValidationError::InvalidSchema)?;
        let base =
            jsonschema::uri::from_str(ROOT_URI).map_err(|_| ToolValidationError::InvalidSchema)?;
        let resource_uri = match schema[root.draft().id_keyword()].as_str() {
            Some(id) => jsonschema::uri::resolve_against(&base.borrow(), id)
                .map_err(|_| ToolValidationError::InvalidSchema)?,
            None => base,
        };
        Ok(Self {
            root,
            resources,
            resource_uri,
        })
    }

    pub(crate) fn is_valid(&self, value: &Value) -> bool {
        self.root.is_valid(value)
    }

    pub(crate) fn branch_is_valid(
        &self,
        path: &str,
        value: &Value,
    ) -> Result<bool, ToolValidationError> {
        // Resolve through the full document so ancestor resource boundaries apply.
        let mut fragment = jsonschema::uri::EncodedBuffer::new();
        fragment.encode_str::<jsonschema::uri::Path>(&path[1..]);
        let reference = self.resource_uri.with_fragment(Some(fragment.as_estr()));
        let branch = json!({"$ref": reference.as_str()});
        jsonschema::options()
            .offline()
            .with_draft(self.root.draft())
            .with_registry(&self.resources)
            .build(&branch)
            .map(|branch| branch.is_valid(value))
            .map_err(|_| ToolValidationError::InvalidSchema)
    }
}
