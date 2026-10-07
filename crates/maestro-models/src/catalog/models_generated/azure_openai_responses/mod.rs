// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod gpt_1;
mod gpt_2;
mod o;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(gpt_1::models_0());
    models.extend(gpt_1::models_10());
    models.extend(gpt_2::models_20());
    models.extend(gpt_2::models_30());
    models.extend(o::models_34());
    models
}
