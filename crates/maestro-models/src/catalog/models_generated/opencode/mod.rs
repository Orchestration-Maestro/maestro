// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod big;
mod claude;
mod gemini;
mod glm;
mod gpt;
mod hy;
mod kimi;
mod minimax;
mod nemotron;
mod qwen;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(big::models_0());
    models.extend(claude::models_1());
    models.extend(gemini::models_9());
    models.extend(glm::models_11());
    models.extend(gpt::models_13());
    models.extend(gpt::models_23());
    models.extend(hy::models_29());
    models.extend(kimi::models_30());
    models.extend(minimax::models_32());
    models.extend(nemotron::models_35());
    models.extend(qwen::models_36());
    models
}
