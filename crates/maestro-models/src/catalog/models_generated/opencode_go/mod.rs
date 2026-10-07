// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod deepseek_v4_flash;
mod glm_5;
mod kimi_k2_5;
mod mimo_v2_omni;
mod minimax_m2_5;
mod qwen3_5_plus;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(deepseek_v4_flash::models());
    models.extend(glm_5::models());
    models.extend(kimi_k2_5::models());
    models.extend(mimo_v2_omni::models());
    models.extend(minimax_m2_5::models());
    models.extend(qwen3_5_plus::models());
    models
}
