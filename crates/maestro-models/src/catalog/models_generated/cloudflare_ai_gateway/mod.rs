// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod claude_3_5_haiku;
mod claude_opus_4_6;
mod gpt_4;
mod gpt_5_5;
mod o1;
mod workers_ai_cf_moonshotai_kimi_k2_5;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(claude_3_5_haiku::models());
    models.extend(claude_opus_4_6::models());
    models.extend(gpt_4::models());
    models.extend(gpt_5_5::models());
    models.extend(o1::models());
    models.extend(workers_ai_cf_moonshotai_kimi_k2_5::models());
    models
}
