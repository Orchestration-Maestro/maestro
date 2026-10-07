// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod deepseek_r1_distill_llama_70b;
mod gemma2_9b_it;
mod groq_compound;
mod llama_3_1_8b_instant;
mod meta_llama_llama_4_maverick_17b_128e_instruct;
mod mistral_saba_24b;
mod moonshotai_kimi_k2_instruct;
mod openai_gpt_oss_120b;
mod qwen_qwq_32b;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(deepseek_r1_distill_llama_70b::models());
    models.extend(gemma2_9b_it::models());
    models.extend(groq_compound::models());
    models.extend(llama_3_1_8b_instant::models());
    models.extend(meta_llama_llama_4_maverick_17b_128e_instruct::models());
    models.extend(mistral_saba_24b::models());
    models.extend(moonshotai_kimi_k2_instruct::models());
    models.extend(openai_gpt_oss_120b::models());
    models.extend(qwen_qwq_32b::models());
    models
}
