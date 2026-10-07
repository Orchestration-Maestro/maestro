// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod minimax_m2_7;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(minimax_m2_7::models());
    models
}
