// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod kimi;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(kimi::models_0());
    models
}
