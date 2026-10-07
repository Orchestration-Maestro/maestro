// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
#[path = "minimax.rs"]
mod minimax_data;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(minimax_data::models_0());
    models
}
