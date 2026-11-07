//! Screenshot observations of the artifact console's line roles, summary hover, follow focus and
//! glyph shapes.
//!
//! The export leaves the pointer and keyboard focus where its scene needs them, so these
//! observations never move the pointer or scroll the page; an element outside the visible page
//! fails instead.

use super::{
    Page, changed, contrast,
    region::{role_pixels, visible},
    require,
};
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

/// Glyph scenes: the scene, the glyph's selector, the bounds of its painted width over height and
/// the shape those bounds name. At the driver's 1.25 device pixel ratio, a right chevron measures
/// 0.67, a down chevron 1.5, the copy icon 0.95 and the check mark 1.45, antialiasing included.
const GLYPHS: [(&str, &str, (f64, f64), &str); 4] = [
    (
        "console-collapsed",
        ".maestro-artifact-summary svg",
        (0.0, 0.75),
        "a right chevron",
    ),
    (
        "console-expanded",
        ".maestro-artifact-summary svg",
        (1.33, f64::INFINITY),
        "a down chevron",
    ),
    (
        "console-copy",
        "copy-button svg",
        (0.85, 1.18),
        "the copy icon",
    ),
    (
        "console-copied",
        "copy-button svg",
        (1.2, f64::INFINITY),
        "a check mark",
    ),
];

/// Observe one console scene.
pub(super) async fn observe(page: &mut Page<'_>, scene: &str) -> io::Result<()> {
    match scene {
        "console-rest" => resting(page).await,
        "console-hover" => hovered(page).await,
        "console-focus" => focused(page).await,
        _ => glyph(page, scene).await,
    }
}

/// The summary and both line kinds paint their resting roles.
async fn resting(page: &mut Page<'_>) -> io::Result<()> {
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
async fn hovered(page: &mut Page<'_>) -> io::Result<()> {
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

/// The glyph of `scene` paints ink whose box has that glyph's shape: the pixels that change when
/// the glyph is hidden span a width over height inside the scene's bounds.
async fn glyph(page: &mut Page<'_>, scene: &str) -> io::Result<()> {
    let (selector, (least, most), shape) = GLYPHS
        .iter()
        .find(|glyph| glyph.0 == scene)
        .map(|&(_, selector, bounds, shape)| (selector, bounds, shape))
        .ok_or_else(|| io::Error::other(format!("unknown paint scene {scene}")))?;
    let rect = visible(page, selector).await?;
    let element = json!(selector).to_string();
    contrast(
        page,
        &format!("[__maestroPaint.element({element})]"),
        "n=>n.style.visibility='hidden'",
    )
    .await?;
    let ink: Option<[f64; 2]> = page.read(&format!("(e=>e&&[e[2]-e[0]+1/devicePixelRatio,e[3]-e[1]+1/devicePixelRatio])(__maestroPaint.extent('decorated','without',{rect}))")).await?;
    let [width, height] =
        ink.ok_or_else(|| io::Error::other(format!("{selector} paints no ink")))?;
    let ratio = width / height;
    require(
        (least..=most).contains(&ratio),
        &format!("{selector} paints {shape} ({width:.2} by {height:.2} pixels)"),
    )?;
    let pixels = changed(page, Some(&element), ("decorated", "without")).await?;
    page.observed(scene, selector, pixels);
    Ok(())
}

/// The keyboard-focused follow control paints its accent fill and on-accent glyph, and its
/// accent outline covers more than a quarter of the band along each edge, so no edge is
/// missing or clipped away. Each band stops short of the rounded corners by the outline's
/// reach, so only the straight part of an edge counts. A quarter, not a half, because a device
/// pixel ratio above one antialiases the outer edge against the page.
async fn focused(page: &mut Page<'_>) -> io::Result<()> {
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
