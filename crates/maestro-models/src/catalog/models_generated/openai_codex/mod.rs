// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod gpt_5_1;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(gpt_5_1::models());
    models
}
