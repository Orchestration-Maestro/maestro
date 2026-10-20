//! Shared selected-pack placement for native sandbox browser witnesses.
use maestro_theme::{BrandMode, ThemeInfo, ThemeOperations};
use maestro_web::SandboxIframe;
use wasm_bindgen::JsCast as _;
/// Selected same-format pack supplied without filesystem access.
pub struct Pack(
    /// Optional same-format selection, falling back to the checked-in pack.
    pub Option<String>,
);
impl ThemeOperations for Pack {
    fn read_to_string(&self, _: &str) -> std::io::Result<String> {
        Ok(self
            .0
            .clone()
            .unwrap_or_else(|| include_str!("../../../../assets/brand/brand.json").into()))
    }
    fn environment(&self, _: &str) -> Option<String> {
        unreachable!()
    }
    fn exists(&self, _: &str) -> bool {
        unreachable!()
    }
    fn read_dir(&self, _: &str) -> std::io::Result<Vec<String>> {
        unreachable!()
    }
    fn sort_by_name(&self, _: &mut [ThemeInfo]) -> std::io::Result<()> {
        unreachable!()
    }
}
/// Construct a mounted owner from the selected brand pack.
#[must_use]
pub(crate) fn adapter() -> SandboxIframe {
    adapter_pack(false, BrandMode::Dark).0
}
/// Materialize a second same-format selection using only existing approved pack values.
#[must_use]
pub(crate) fn adapter_pack(alternate: bool, mode: BrandMode) -> (SandboxIframe, String) {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&info.to_string().into());
    }));
    let mut json: serde_json::Value =
        serde_json::from_str(include_str!("../../../../assets/brand/brand.json")).unwrap();
    if alternate {
        json["modes"]["dark"]["colors"]["background"] = "oxide".into();
        json["modes"]["light"]["colors"]["panel"] = "burnished-brass".into();
        json["fonts"]["mono"] = json["fonts"]["body"].clone();
    }
    let pack = maestro_theme::load_brand_pack("brand.json", &Pack(Some(json.to_string()))).unwrap();
    let presentation = pack.presentation(mode).unwrap();
    let css = serde_json::to_string(&presentation.css_properties()).unwrap();
    let document = web_sys::window().unwrap().document().unwrap();
    let parent = document.create_element("div").unwrap();
    parent.set_attribute("data-sandbox-test", "").unwrap();
    document.body().unwrap().append_child(&parent).unwrap();
    (
        SandboxIframe::new(parent.unchecked_into(), &presentation).unwrap(),
        css,
    )
}
