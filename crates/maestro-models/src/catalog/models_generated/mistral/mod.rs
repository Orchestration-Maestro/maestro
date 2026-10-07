// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod codestral_latest;
mod devstral_2512;
mod labs_devstral_small_2512;
mod magistral_medium_latest;
mod ministral_3b_latest;
mod mistral_large_2411;
mod mistral_small_2603;
mod open_mistral_7b;
mod pixtral_12b;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(codestral_latest::models());
    models.extend(devstral_2512::models());
    models.extend(labs_devstral_small_2512::models());
    models.extend(magistral_medium_latest::models());
    models.extend(ministral_3b_latest::models());
    models.extend(mistral_large_2411::models());
    models.extend(mistral_small_2603::models());
    models.extend(open_mistral_7b::models());
    models.extend(pixtral_12b::models());
    models
}
