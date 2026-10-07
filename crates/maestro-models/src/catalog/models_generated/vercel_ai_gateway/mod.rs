// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod alibaba;
mod anthropic;
mod arcee_ai;
mod bytedance;
mod cohere;
mod deepseek;
mod google;
mod inception;
mod kwaipilot;
mod meituan;
mod meta;
mod minimax;
mod mistral;
mod moonshotai;
mod nvidia;
mod openai_1;
mod openai_2;
mod perplexity;
mod xai;
mod xiaomi;
mod zai;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(alibaba::models_0());
    models.extend(alibaba::models_10());
    models.extend(anthropic::models_18());
    models.extend(anthropic::models_28());
    models.extend(arcee_ai::models_30());
    models.extend(bytedance::models_32());
    models.extend(cohere::models_33());
    models.extend(deepseek::models_34());
    models.extend(google::models_42());
    models.extend(google::models_52());
    models.extend(inception::models_53());
    models.extend(kwaipilot::models_55());
    models.extend(meituan::models_56());
    models.extend(meta::models_57());
    models.extend(minimax::models_64());
    models.extend(mistral::models_71());
    models.extend(moonshotai::models_81());
    models.extend(nvidia::models_87());
    models.extend(openai_1::models_89());
    models.extend(openai_1::models_99());
    models.extend(openai_2::models_109());
    models.extend(openai_2::models_119());
    models.extend(perplexity::models_126());
    models.extend(xai::models_128());
    models.extend(xai::models_138());
    models.extend(xiaomi::models_145());
    models.extend(zai::models_149());
    models.extend(zai::models_159());
    models
}
