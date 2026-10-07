// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod claude_1;
mod claude_2;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(claude_1::models_0());
    models.extend(claude_1::models_10());
    models.extend(claude_2::models_20());
    models
}
