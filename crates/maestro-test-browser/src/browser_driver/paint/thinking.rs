//! Screenshot observations of the reasoning disclosure's header roles and chevron.
//!
//! The export leaves the pointer and the disclosure where its scene needs them, so these
//! observations never move the pointer or scroll the page; a header outside the visible page
//! fails.

use super::{
    Page,
    region::{DOWN_CHEVRON, RIGHT_CHEVRON, shape},
    require, without,
};
use serde_json::json;
use std::io;

/// The header's literal label.
const LABEL: &str = ".maestro-thinking-header > span:last-child";

/// The header's chevron glyph, which paints the header's text colour.
const CHEVRON: &str = ".maestro-thinking-chevron svg";

/// Observe one reasoning scene.
pub(super) async fn observe(page: &mut Page<'_>, scene: &str) -> io::Result<()> {
    match scene {
        "thinking-rest" => {
            header_paints(page, scene, "muted").await?;
            shape(page, scene, CHEVRON, RIGHT_CHEVRON, "a right chevron").await
        }
        "thinking-hover" => {
            require(
                page.read("document.querySelector('.maestro-thinking-header').matches(':hover')")
                    .await?,
                "the pointer must rest on the reasoning header",
            )?;
            header_paints(page, scene, "text").await
        }
        _ => shape(page, scene, CHEVRON, DOWN_CHEVRON, "a down chevron").await,
    }
}

/// The label and the chevron paint exactly the brand `role`: repainting either in that role's
/// colour changes no pixel inside its box, while making it transparent changes some.
async fn header_paints(page: &mut Page<'_>, scene: &str, role: &str) -> io::Result<()> {
    for selector in [LABEL, CHEVRON] {
        let element = json!(selector).to_string();
        let nodes = format!("[__maestroPaint.element({element})]");
        let colour = format!(
            "n=>n.style.setProperty('color',JSON.parse(globalThis.colors)[{}],'important')",
            json!(role)
        );
        let differs = without(page, &nodes, &colour, Some(&element)).await?;
        require(
            differs == 0,
            &format!("{selector} paints {role} ({differs} pixels differ)"),
        )?;
        let hidden = "n=>n.style.setProperty('color','transparent','important')";
        let painted = without(page, &nodes, hidden, Some(&element)).await?;
        require(painted > 0, &format!("{selector} paints ink"))?;
        page.observed(scene, selector, painted);
    }
    Ok(())
}
