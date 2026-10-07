// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod deepseek;
mod gemma;
#[path = "groq.rs"]
mod groq_data;
mod llama;
mod meta_llama;
mod mistral;
mod moonshotai;
mod openai;
mod qwen;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(deepseek::models_0());
    models.extend(gemma::models_1());
    models.extend(groq_data::models_2());
    models.extend(llama::models_4());
    models.extend(meta_llama::models_8());
    models.extend(mistral::models_10());
    models.extend(moonshotai::models_11());
    models.extend(openai::models_13());
    models.extend(qwen::models_16());
    models
}
