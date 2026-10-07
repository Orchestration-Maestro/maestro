// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod claude_haiku_4_5;
mod gemini_2_5_pro;
mod gpt_4_1;
mod gpt_5_3_codex;
mod grok_code_fast_1;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(claude_haiku_4_5::models());
    models.extend(gemini_2_5_pro::models());
    models.extend(gpt_4_1::models());
    models.extend(gpt_5_3_codex::models());
    models.extend(grok_code_fast_1::models());
    models
}
