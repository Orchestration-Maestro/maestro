//! Visible boxes and their pixel counts, shared by the artifact scenes.

use super::{Page, require};
use serde_json::{Value, json};
use std::io;

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
