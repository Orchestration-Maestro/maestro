// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod deepseek;
mod glm;
mod kimi;
mod mimo;
mod minimax;
mod qwen;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(deepseek::models_0());
    models.extend(glm::models_2());
    models.extend(kimi::models_4());
    models.extend(mimo::models_6());
    models.extend(minimax::models_10());
    models.extend(qwen::models_12());
    models
}
