// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
#[path = "deepseek.rs"]
mod deepseek_data;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(deepseek_data::models_0());
    models
}
