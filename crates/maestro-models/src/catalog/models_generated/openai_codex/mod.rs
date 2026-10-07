// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod gpt;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(gpt::models_0());
    models
}
