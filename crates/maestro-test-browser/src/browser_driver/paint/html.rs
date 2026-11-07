//! Screenshot observations of the HTML artifact's source listing.
//!
//! The scenes observe the first listing as the export left it, so they never move the pointer
//! or scroll; a listing outside the visible page fails.

use super::{
    Page, changed, contrast,
    region::{area, role_pixels, visible},
    require,
};
use serde_json::json;
use std::io;

/// The observed listing.
const LISTING: &str = ".maestro-html-source";

/// The shown listing paints the background role over more than half of its box, and its text
/// paints inside the bounded body and nowhere outside it. The capture stays as `html-source`
/// for `again`.
pub(super) async fn shown(page: &mut Page<'_>) -> io::Result<()> {
    let scene = "html-source";
    let rect = visible(page, LISTING).await?;
    page.shot(scene).await?;
    let background = role_pixels(page, scene, &rect, "background").await?;
    let pixels = area(page, &rect).await?;
    require(
        background * 2 > pixels,
        &format!(
            "the listing paints the background role over its box ({background} of {pixels} pixels)"
        ),
    )?;
    page.observed(scene, "background", background);
    let listing = format!("__maestroPaint.element({})", json!(LISTING));
    contrast(
        page,
        &format!("[{listing}.querySelector('code')]"),
        "n=>n.style.visibility='hidden'",
    )
    .await?;
    let body = format!("{listing}.closest('.maestro-html-artifact')");
    let inside = changed(page, Some(&body), ("decorated", "without")).await?;
    require(inside > 0, "the listing text paints inside the body")?;
    let outside: u64 = page
        .read(&format!(
            "__maestroPaint.differ('decorated','without',__maestroPaint.box({body}),true)"
        ))
        .await?;
    require(
        outside == 0,
        &format!("the listing text paints nowhere outside the body ({outside} pixels)"),
    )?;
    page.observed(scene, "text", inside);
    Ok(())
}

/// The listing paints exactly what the last `html-source` capture showed inside its box, so a
/// listing hidden and shown again keeps its scroll position and content.
pub(super) async fn again(page: &mut Page<'_>) -> io::Result<()> {
    let scene = "html-source-again";
    let rect = visible(page, LISTING).await?;
    page.shot(scene).await?;
    let differs: u64 = page
        .read(&format!(
            "__maestroPaint.differ('html-source',{},{rect})",
            json!(scene)
        ))
        .await?;
    require(
        differs == 0,
        &format!("the listing paints as it did before it was hidden ({differs} pixels differ)"),
    )?;
    let pixels = area(page, &rect).await?;
    page.observed(scene, "unchanged", pixels);
    Ok(())
}
