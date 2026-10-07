// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod claude;
mod gpt;
mod moonshotai;
mod nvidia;
mod o;
mod zai_org;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(claude::models_0());
    models.extend(claude::models_10());
    models.extend(gpt::models_15());
    models.extend(gpt::models_25());
    models.extend(o::models_26());
    models.extend(moonshotai::models_31());
    models.extend(nvidia::models_33());
    models.extend(zai_org::models_34());
    models
}
