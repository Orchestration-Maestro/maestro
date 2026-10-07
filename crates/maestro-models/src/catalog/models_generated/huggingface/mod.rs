// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod deepseek_ai;
mod minimaxai;
mod moonshotai;
mod qwen;
mod xiaomimimo;
mod zai_org;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(minimaxai::models_0());
    models.extend(qwen::models_3());
    models.extend(xiaomimimo::models_9());
    models.extend(deepseek_ai::models_10());
    models.extend(moonshotai::models_13());
    models.extend(zai_org::models_18());
    models
}
