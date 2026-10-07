// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod glm;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(glm::models_0());
    models
}
