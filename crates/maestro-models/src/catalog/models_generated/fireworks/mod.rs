// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod deepseek;
mod glm;
mod gpt;
mod kimi;
mod minimax;
mod qwen;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(deepseek::models_0());
    models.extend(glm::models_3());
    models.extend(gpt::models_8());
    models.extend(kimi::models_10());
    models.extend(minimax::models_14());
    models.extend(qwen::models_17());
    models.extend(kimi::models_18());
    models
}
