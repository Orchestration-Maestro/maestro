//! Screenshot observations of the Markdown block's link hover, syntax roles and brand roles.

use super::{Page, changed, require, reveal};
use serde_json::{Value, json};
use std::io;

/// The brand roles that token names resolve to.
const ROLES: &str = include_str!("../../../../maestro-theme/assets/theme/brand-roles.json");

/// The authored link of the hover scene.
const LINK: &str = ".maestro-markdown a";

/// Highlighted token classes, each painted in the role its theme token names.
const TOKENS: [&str; 9] = [
    "comment",
    "keyword",
    "function",
    "variable",
    "string",
    "number",
    "type",
    "operator",
    "punctuation",
];

/// Selectors that paint a brand role in every brand and thinking state; the paragraph's role
/// follows the thinking state.
const BRAND: [(&str, &str); 6] = [
    ("a", "accent"),
    ("code:not(.hljs)", "panel"),
    ("blockquote", "border"),
    ("th", "panel"),
    (".katex-error", "error"),
    (".maestro-code", "border"),
];

/// Per-channel difference from a role's colour that still counts as painting it.
const TOLERANCE: u8 = 1;

/// Observe one Markdown scene.
pub(super) async fn observe(page: &mut Page<'_>, scene: &str) -> io::Result<()> {
    match scene {
        "markdown-hover" => hover(page, scene).await,
        "markdown-syntax" => syntax(page, scene).await,
        _ => brand(page, scene).await,
    }
}

/// Hovering the link repaints its text and leaves its image and inline-code background as they
/// were.
async fn hover(page: &mut Page<'_>, scene: &str) -> io::Result<()> {
    let link = json!(LINK).to_string();
    let image = json!(format!("{LINK} img")).to_string();
    let code = json!(format!("{LINK} code")).to_string();
    let text = json!(format!("{LINK} span")).to_string();
    reveal(page, &link).await?;
    page.evaluate("Promise.all([...document.querySelectorAll('.maestro-markdown a img')].map(image => image.decode()))").await?;
    page.shot("rest").await?;
    let at: (f64, f64) = page
        .read(&format!(
            "(([l,t]) => [l+2,t+2])(__maestroPaint.box({link}))"
        ))
        .await?;
    let root = page.root_context()?;
    page.pointer(&root, at, false).await?;
    page.shot("hovered").await?;
    let between = ("rest", "hovered");
    let moved = changed(page, Some(&image), between).await?;
    require(moved == 0, "the linked image's paint must not change")?;
    let corner: bool = page.read(&format!("(([l,t]) => JSON.stringify(__maestroPaint.pixel('rest',[l+1,t+1])) === JSON.stringify(__maestroPaint.pixel('hovered',[l+1,t+1])))(__maestroPaint.box({code}))")).await?;
    require(corner, "the inline-code background must not change")?;
    let repainted = changed(page, Some(&text), between).await?;
    require(repainted > 0, "the link text must repaint on hover")?;
    page.observed(scene, "link", repainted);
    Ok(())
}

/// Every token class paints pixels of the role its theme token names.
async fn syntax(page: &mut Page<'_>, scene: &str) -> io::Result<()> {
    let roles: Value = serde_json::from_str(ROLES).map_err(io::Error::other)?;
    for name in TOKENS {
        let token = format!("syntax{}{}", name[..1].to_uppercase(), &name[1..]);
        let role = roles["colors"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|pair| pair[0] == token.as_str())
            .and_then(|pair| pair[1].as_str())
            .ok_or_else(|| io::Error::other(format!("no brand role for {token}")))?;
        painted(
            page,
            scene,
            &format!(".maestro-markdown .hljs-{name}"),
            role,
        )
        .await?;
    }
    Ok(())
}

/// The paragraph, link, inline code, quote border, table header, formula error and listing border
/// paint the roles the selected pack and thinking state give them.
async fn brand(page: &mut Page<'_>, scene: &str) -> io::Result<()> {
    let thinking: bool = page
        .read("getComputedStyle(document.querySelector('.maestro-markdown')).fontStyle==='italic'")
        .await?;
    painted(
        page,
        scene,
        ".maestro-markdown p",
        if thinking { "muted" } else { "text" },
    )
    .await?;
    for (selector, role) in BRAND {
        painted(page, scene, &format!(".maestro-markdown {selector}"), role).await?;
    }
    page.evaluate("(() => {const v = document.querySelector('.maestro-markdown').parentElement.parentElement; v.scrollTop = 0;})()").await?;
    Ok(())
}

/// The first `selector` match paints pixels of `role` inside its box, kept within the reading
/// viewport and the visible page.
async fn painted(page: &mut Page<'_>, scene: &str, selector: &str, role: &str) -> io::Result<()> {
    let element = json!(selector).to_string();
    reveal(page, &element).await?;
    let rect: Value = page.read(&format!("(() => {{const b=__maestroPaint.element({element}).getBoundingClientRect(),m=document.querySelector('.maestro-markdown'),c=(m.closest('.maestro-reading-viewport')??m.parentElement.parentElement).getBoundingClientRect();const r=[Math.max(b.left,c.left,0),Math.max(b.top,c.top,0),Math.min(b.right,c.right,innerWidth),Math.min(b.bottom,c.bottom,innerHeight)];return r[2]>r[0]&&r[3]>r[1]?r:null;}})()")).await?;
    require(
        !rect.is_null(),
        &format!("{selector} must have visible geometry"),
    )?;
    page.shot(selector).await?;
    let pixels: u64 = page
        .read(&format!(
            "__maestroPaint.count({},{rect},__maestroPaint.colour({}),{TOLERANCE})",
            json!(selector),
            json!(role)
        ))
        .await?;
    require(pixels > 0, &format!("{selector} must paint {role} pixels"))?;
    page.observed(scene, selector, pixels);
    Ok(())
}
