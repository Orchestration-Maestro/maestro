// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod google;
mod meta;
mod moonshotai;
mod nvidia;
mod openai;
mod zai_org;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(google::models_0());
    models.extend(meta::models_1());
    models.extend(moonshotai::models_2());
    models.extend(nvidia::models_4());
    models.extend(openai::models_5());
    models.extend(zai_org::models_7());
    models
}
