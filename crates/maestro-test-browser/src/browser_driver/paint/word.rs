//! Screenshot observation of a Word artifact's page.
//!
//! The scene observes the first page as the export left it in its scroll box, so it never moves
//! the pointer or scrolls.

use super::{Page, region::area, require};
use serde_json::Value;
use std::io;

/// The first Word page's box clipped to its artifact's scroll box and the visible page, or null
/// when nothing of it shows.
const SHEET: &str = "(() => {const s=document.querySelector('section.docx'),a=s?.closest('.maestro-docx-artifact');if(!a)return null;const b=s.getBoundingClientRect(),c=a.getBoundingClientRect(),r=[Math.max(b.left,c.left,0),Math.max(b.top,c.top,0),Math.min(b.right,c.right,innerWidth),Math.min(b.bottom,c.bottom,innerHeight)];return r[2]>r[0]&&r[3]>r[1]?r:null;})()";

/// Highest channel value of a pixel that counts as black ink. Black text antialiased over white
/// paints dark pixels, tinted when antialiasing works per subpixel, and a thin glyph may never
/// reach pure black.
const INK: u8 = 0x60;

/// The visible part of the first Word page paints white over more than half of its pixels, so
/// the page and not the artifact's background shows, and its text paints black ink.
pub(super) async fn sheet(page: &mut Page<'_>) -> io::Result<()> {
    let scene = "word-page";
    let rect: Value = page.read(SHEET).await?;
    require(!rect.is_null(), "a Word page must be visible")?;
    page.shot(scene).await?;
    let [white, black]: [u64; 2] = page
        .read(&format!(
            "[__maestroPaint.count('{scene}',{rect},[255,255,255]),__maestroPaint.dark('{scene}',{rect},{INK})]"
        ))
        .await?;
    let pixels = area(page, &rect).await?;
    require(
        white * 2 > pixels,
        &format!("the Word page paints white over its box ({white} of {pixels} pixels)"),
    )?;
    page.observed(scene, "white", white);
    require(black > 0, "the Word page text paints black ink")?;
    page.observed(scene, "black", black);
    Ok(())
}
