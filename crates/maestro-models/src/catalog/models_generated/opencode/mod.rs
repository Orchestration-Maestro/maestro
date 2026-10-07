// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod big_pickle;
mod claude_haiku_4_5;
mod gemini_3_flash;
mod glm_5;
mod gpt_5;
mod gpt_5_4;
mod hy3_preview_free;
mod kimi_k2_5;
mod minimax_m2_5;
mod nemotron_3_super_free;
mod qwen3_5_plus;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(big_pickle::models());
    models.extend(claude_haiku_4_5::models());
    models.extend(gemini_3_flash::models());
    models.extend(glm_5::models());
    models.extend(gpt_5::models());
    models.extend(gpt_5_4::models());
    models.extend(hy3_preview_free::models());
    models.extend(kimi_k2_5::models());
    models.extend(minimax_m2_5::models());
    models.extend(nemotron_3_super_free::models());
    models.extend(qwen3_5_plus::models());
    models
}
