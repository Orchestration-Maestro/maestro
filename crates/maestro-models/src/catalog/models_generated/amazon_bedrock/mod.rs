// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod amazon;
mod anthropic_1;
mod anthropic_2;
mod anthropic_3;
mod deepseek;
mod google;
mod meta;
mod minimax;
mod mistral;
mod moonshot;
mod moonshotai;
mod nvidia;
mod openai;
mod qwen;
mod writer;
mod zai;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(amazon::models_0());
    models.extend(anthropic_1::models_5());
    models.extend(anthropic_1::models_15());
    models.extend(deepseek::models_21());
    models.extend(anthropic_1::models_24());
    models.extend(anthropic_2::models_26());
    models.extend(anthropic_2::models_36());
    models.extend(google::models_38());
    models.extend(meta::models_40());
    models.extend(minimax::models_50());
    models.extend(mistral::models_53());
    models.extend(moonshot::models_62());
    models.extend(moonshotai::models_63());
    models.extend(nvidia::models_64());
    models.extend(openai::models_68());
    models.extend(qwen::models_72());
    models.extend(anthropic_2::models_79());
    models.extend(anthropic_3::models_85());
    models.extend(writer::models_88());
    models.extend(zai::models_90());
    models
}
