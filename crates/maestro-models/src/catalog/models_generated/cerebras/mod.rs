// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod gpt;
mod llama;
mod qwen;
mod zai;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(gpt::models_0());
    models.extend(llama::models_1());
    models.extend(qwen::models_2());
    models.extend(zai::models_3());
    models
}
