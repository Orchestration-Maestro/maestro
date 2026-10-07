// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod codestral;
mod devstral;
mod magistral;
mod ministral;
#[path = "mistral.rs"]
mod mistral_data;
mod mixtral;
mod pixtral;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(codestral::models_0());
    models.extend(devstral::models_1());
    models.extend(magistral::models_7());
    models.extend(ministral::models_9());
    models.extend(mistral_data::models_11());
    models.extend(mistral_data::models_21());
    models.extend(mixtral::models_24());
    models.extend(pixtral::models_26());
    models
}
