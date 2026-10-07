// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod gemini_1;
mod gemini_2;
mod gemma;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(gemini_1::models_0());
    models.extend(gemini_1::models_10());
    models.extend(gemini_2::models_18());
    models.extend(gemma::models_24());
    models
}
