//! Screenshot observations of the artifact console's line roles, summary hover and follow focus.
//!
//! The export leaves the pointer and keyboard focus where its scene needs them, so these
//! observations never move the pointer or scroll the page; an element outside the visible page
//! fails instead.

use super::{Page, require};
use serde_json::{Value, json};
use std::io;

/// Console selectors whose resting paint must reach the selected brand role.
const RESTING: [(&str, &str); 3] = [
    (".maestro-artifact-summary", "muted"),
    (".maestro-artifact-log", "muted"),
    (".maestro-artifact-error", "error"),
];

/// Distance of the focus outline beyond the control's edges, then its width, in CSS pixels.
const OUTLINE: (f64, f64) = (2.0, 2.0);

/// Inset that keeps the follow control's rounded corners out of its glyph count, in CSS pixels.
const GLYPH_INSET: f64 = 4.0;

/// The first `selector` match's visible box as `[left, top, right, bottom]`, failing when it
/// has no area or reaches outside the visible page.
async fn visible(page: &mut Page<'_>, selector: &str) -> io::Result<Value> {
    let rect: Value = page.read(&format!("(() => {{const b=__maestroPaint.element({}).getBoundingClientRect();return b.width>0&&b.height>0&&b.left>=0&&b.top>=0&&b.right<=innerWidth&&b.bottom<=innerHeight?[b.left,b.top,b.right,b.bottom]:null;}})()", json!(selector))).await?;
    require(!rect.is_null(), &format!("{selector} must be visible"))?;
    Ok(rect)
}

/// Count pixels of the brand `role` inside `rect` of the capture `label`.
async fn role_pixels(
    page: &mut Page<'_>,
    label: &str,
    rect: &Value,
    role: &str,
) -> io::Result<u64> {
    page.read(&format!(
        "__maestroPaint.count({},{rect},__maestroPaint.colour({}))",
        json!(label),
        json!(role)
    ))
    .await
}

/// The summary and both line kinds paint their resting roles.
pub(super) async fn resting(page: &mut Page<'_>) -> io::Result<()> {
    let scene = "console-rest";
    page.shot(scene).await?;
    for (selector, role) in RESTING {
        let rect = visible(page, selector).await?;
        let pixels = role_pixels(page, scene, &rect, role).await?;
        require(pixels > 0, &format!("{selector} paints {role}"))?;
        page.observed(scene, selector, pixels);
    }
    Ok(())
}

/// The summary under the native pointer paints the text role.
pub(super) async fn hovered(page: &mut Page<'_>) -> io::Result<()> {
    let scene = "console-hover";
    let selector = ".maestro-artifact-summary";
    require(
        page.read(&format!(
            "document.querySelector({}).matches(':hover')",
            json!(selector)
        ))
        .await?,
        "the pointer must rest on the console summary",
    )?;
    let rect = visible(page, selector).await?;
    page.shot(scene).await?;
    let pixels = role_pixels(page, scene, &rect, "text").await?;
    require(pixels > 0, "the hovered summary paints text")?;
    page.observed(scene, selector, pixels);
    Ok(())
}

/// The keyboard-focused follow control paints its accent fill and on-accent glyph, and its
/// accent outline covers more than a quarter of the band along each edge, so no edge is
/// missing or clipped away. Each band stops short of the rounded corners by the outline's
/// reach, so only the straight part of an edge counts. A quarter, not a half, because a device
/// pixel ratio above one antialiases the outer edge against the page.
pub(super) async fn focused(page: &mut Page<'_>) -> io::Result<()> {
    let scene = "console-focus";
    let selector = ".maestro-artifact-follow";
    require(
        page.read(&format!(
            "document.activeElement?.matches({})===true",
            json!(selector)
        ))
        .await?,
        "keyboard focus must reach the follow control",
    )?;
    let rect = visible(page, selector).await?;
    page.shot(scene).await?;
    let fill = role_pixels(page, scene, &rect, "accent").await?;
    require(
        fill > 0,
        "the pressed follow control paints its accent fill",
    )?;
    page.observed(scene, "fill", fill);
    let glyph: Value = page.read(&format!("(([l,t,r,b]) => [l+{GLYPH_INSET},t+{GLYPH_INSET},r-{GLYPH_INSET},b-{GLYPH_INSET}])({rect})")).await?;
    let glyph = role_pixels(page, scene, &glyph, "on-accent").await?;
    require(glyph > 0, "the follow glyph paints on-accent")?;
    page.observed(scene, "glyph", glyph);
    let (offset, width) = OUTLINE;
    let edges: [[u64; 2]; 4] = page.read(&format!("(([l,t,r,b]) => {{const o={offset},w={width},n=o+w,d=devicePixelRatio,accent=__maestroPaint.colour('accent');return [[l+n,t-n,r-n,t-o],[r+o,t+n,r+n,b-n],[l+n,b+o,r-n,b+n],[l-n,t+n,l-o,b-n]].map(e=>[__maestroPaint.count({},e,accent),Math.round((e[2]-e[0])*(e[3]-e[1])*d*d)]);}})({rect})", json!(scene))).await?;
    for (edge, [pixels, area]) in ["top", "right", "bottom", "left"].into_iter().zip(edges) {
        require(
            pixels * 4 > area,
            &format!(
                "the focus outline paints accent along the {edge} edge ({pixels} of {area} pixels)"
            ),
        )?;
        page.observed(scene, edge, pixels);
    }
    Ok(())
}
