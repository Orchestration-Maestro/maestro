// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod mimo;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(mimo::models_0());
    models
}
