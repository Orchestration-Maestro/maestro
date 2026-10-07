// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod deepseek_ai_deepseek_r1_0528;
mod minimaxai_minimax_m2_1;
mod moonshotai_kimi_k2_instruct;
mod qwen_qwen3_235b_a22b_thinking_2507;
mod xiaomimimo_mimo_v2_flash;
mod zai_org_glm_4_7;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(minimaxai_minimax_m2_1::models());
    models.extend(qwen_qwen3_235b_a22b_thinking_2507::models());
    models.extend(xiaomimimo_mimo_v2_flash::models());
    models.extend(deepseek_ai_deepseek_r1_0528::models());
    models.extend(moonshotai_kimi_k2_instruct::models());
    models.extend(zai_org_glm_4_7::models());
    models
}
