// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod gemini;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(gemini::models_0());
    models.extend(gemini::models_10());
    models
}
