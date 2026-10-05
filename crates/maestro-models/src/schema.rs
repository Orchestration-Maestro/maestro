//! Offline checker integration retaining enclosing dialect and reference scope.

use crate::ToolValidationError;
use serde_json::Value;

pub(crate) struct Schema {
    root: jsonschema::Validator,
    branches: jsonschema::ValidatorMap,
}

impl Schema {
    pub(crate) fn build(schema: &Value) -> Result<Self, ToolValidationError> {
        let options = jsonschema::options().offline();
        let root = options
            .build(schema)
            .map_err(|_| ToolValidationError::InvalidSchema)?;
        let branches = options
            .build_map(schema)
            .map_err(|_| ToolValidationError::InvalidSchema)?;
        Ok(Self { root, branches })
    }

    pub(crate) fn is_valid(&self, value: &Value) -> bool {
        self.root.is_valid(value)
    }

    pub(crate) fn branch_is_valid(
        &self,
        path: &str,
        value: &Value,
    ) -> Result<bool, ToolValidationError> {
        self.branches
            .get(path)
            .map(|branch| branch.is_valid(value))
            .ok_or(ToolValidationError::InvalidSchema)
    }
}
