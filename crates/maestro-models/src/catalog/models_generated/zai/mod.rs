// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod glm_4_5_air;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(glm_4_5_air::models());
    models
}
