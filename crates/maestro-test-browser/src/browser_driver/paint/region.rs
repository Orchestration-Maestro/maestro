//! Visible boxes, their pixel counts and glyph shapes, shared by the scenes.

use super::{Page, changed, contrast, require};
use serde_json::{Value, json};
use std::io;

/// Bounds of a right chevron's painted width over height, which measures 0.67 at the driver's
/// 1.25 device pixel ratio, antialiasing included.
pub(super) const RIGHT_CHEVRON: (f64, f64) = (0.0, 0.75);

/// Bounds of a down chevron's painted width over height, which measures 1.5 at the driver's
/// 1.25 device pixel ratio, antialiasing included.
pub(super) const DOWN_CHEVRON: (f64, f64) = (1.33, f64::INFINITY);

/// The first `selector` match's visible box as `[left, top, right, bottom]`, failing when it
/// has no area or reaches outside the visible page.
pub(super) async fn visible(page: &mut Page<'_>, selector: &str) -> io::Result<Value> {
    let rect: Value = page.read(&format!("(() => {{const b=__maestroPaint.element({}).getBoundingClientRect();return b.width>0&&b.height>0&&b.left>=0&&b.top>=0&&b.right<=innerWidth&&b.bottom<=innerHeight?[b.left,b.top,b.right,b.bottom]:null;}})()", json!(selector))).await?;
    require(!rect.is_null(), &format!("{selector} must be visible"))?;
    Ok(rect)
}

/// The device pixels a capture holds inside `rect`.
pub(super) async fn area(page: &mut Page<'_>, rect: &Value) -> io::Result<u64> {
    page.read(&format!(
        "(([l,t,r,b]) => Math.round((r-l)*(b-t)*devicePixelRatio**2))({rect})"
    ))
    .await
}

/// Count pixels of the brand `role` inside `rect` of the capture `label`.
pub(super) async fn role_pixels(
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

/// The first `selector` match paints ink whose box has the shape `name`: the pixels that change
/// when the glyph is hidden span a width over height inside `least..=most`.
pub(super) async fn shape(
    page: &mut Page<'_>,
    scene: &str,
    selector: &str,
    (least, most): (f64, f64),
    name: &str,
) -> io::Result<()> {
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
        &format!("{selector} paints {name} ({width:.2} by {height:.2} pixels)"),
    )?;
    let pixels = changed(page, Some(&element), ("decorated", "without")).await?;
    page.observed(scene, selector, pixels);
    Ok(())
}
