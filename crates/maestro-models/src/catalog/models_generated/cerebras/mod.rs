// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod gpt_oss_120b;
mod llama3_1_8b;
mod qwen_3_235b_a22b_instruct_2507;
mod zai_glm_4_7;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(gpt_oss_120b::models());
    models.extend(llama3_1_8b::models());
    models.extend(qwen_3_235b_a22b_instruct_2507::models());
    models.extend(zai_glm_4_7::models());
    models
}
