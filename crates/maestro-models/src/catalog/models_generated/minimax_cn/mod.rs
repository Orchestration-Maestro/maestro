// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod minimax;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(minimax::models_0());
    models
}
