// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod amazon_nova_2_lite_v1_0;
mod anthropic_claude_3_5_haiku_20241022_v1_0;
mod anthropic_claude_opus_4_7;
mod au_anthropic_claude_opus_4_6_v1;
mod deepseek_r1_v1_0;
mod eu_anthropic_claude_haiku_4_5_20251001_v1_0;
mod global_anthropic_claude_haiku_4_5_20251001_v1_0;
mod google_gemma_3_27b_it;
mod meta_llama3_1_405b_instruct_v1_0;
mod minimax_minimax_m2;
mod mistral_devstral_2_123b;
mod moonshot_kimi_k2_thinking;
mod moonshotai_kimi_k2_5;
mod nvidia_nemotron_nano_12b_v2;
mod openai_gpt_oss_120b_1_0;
mod qwen_qwen3_235b_a22b_2507_v1_0;
mod us_anthropic_claude_haiku_4_5_20251001_v1_0;
mod writer_palmyra_x4_v1_0;
mod zai_glm_4_7;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(amazon_nova_2_lite_v1_0::models());
    models.extend(anthropic_claude_3_5_haiku_20241022_v1_0::models());
    models.extend(anthropic_claude_opus_4_7::models());
    models.extend(au_anthropic_claude_opus_4_6_v1::models());
    models.extend(deepseek_r1_v1_0::models());
    models.extend(eu_anthropic_claude_haiku_4_5_20251001_v1_0::models());
    models.extend(global_anthropic_claude_haiku_4_5_20251001_v1_0::models());
    models.extend(google_gemma_3_27b_it::models());
    models.extend(meta_llama3_1_405b_instruct_v1_0::models());
    models.extend(minimax_minimax_m2::models());
    models.extend(mistral_devstral_2_123b::models());
    models.extend(moonshot_kimi_k2_thinking::models());
    models.extend(moonshotai_kimi_k2_5::models());
    models.extend(nvidia_nemotron_nano_12b_v2::models());
    models.extend(openai_gpt_oss_120b_1_0::models());
    models.extend(qwen_qwen3_235b_a22b_2507_v1_0::models());
    models.extend(us_anthropic_claude_haiku_4_5_20251001_v1_0::models());
    models.extend(writer_palmyra_x4_v1_0::models());
    models.extend(zai_glm_4_7::models());
    models
}
