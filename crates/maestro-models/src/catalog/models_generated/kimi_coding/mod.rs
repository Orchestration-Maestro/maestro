// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod kimi_for_coding;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(kimi_for_coding::models());
    models
}
