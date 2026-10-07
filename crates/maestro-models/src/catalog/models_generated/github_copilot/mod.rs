// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod claude;
mod gemini;
mod gpt_1;
mod gpt_2;
mod grok;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(claude::models_0());
    models.extend(gemini::models_7());
    models.extend(gpt_1::models_11());
    models.extend(gpt_1::models_21());
    models.extend(gpt_2::models_24());
    models.extend(grok::models_25());
    models
}
