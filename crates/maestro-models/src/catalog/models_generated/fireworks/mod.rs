// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod accounts_fireworks_models_deepseek_v3p1;
mod accounts_fireworks_models_kimi_k2_instruct;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(accounts_fireworks_models_deepseek_v3p1::models());
    models.extend(accounts_fireworks_models_kimi_k2_instruct::models());
    models
}
