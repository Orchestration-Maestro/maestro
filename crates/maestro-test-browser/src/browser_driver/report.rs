//! Real painting observations for the committed tool report.

use super::{Browser, assets, cleanup, page::Page, report_scripts, session};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{io, path::Path};

/// Collected report assertion failures, preserving every independent observation.
#[derive(Default)]
struct Observations(Vec<String>);

impl Observations {
    /// Emit one actual-versus-expected observation without stopping later assertions.
    fn compare(&mut self, name: &str, actual: &Value, expected: &Value) {
        if actual == expected {
            println!("PASS {name}");
        } else {
            println!("FAIL {name}");
            self.0.push(format!("{name}: {actual} != {expected}"));
        }
    }

    /// Fail the report only after all reachable observations have run.
    fn finish(self) -> io::Result<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(io::Error::other(self.0.join("; ")))
        }
    }
}

/// Decoded browser screenshot, using its actual bitmap width for row geometry.
#[derive(Deserialize)]
struct Painted {
    /// Straight RGBA bytes from the browser image decoder.
    pixels: Vec<u8>,
    /// Raster width in device pixels.
    width: usize,
}

/// Run the committed tool report in the pinned executable on one confined HTTP endpoint,
/// answering its two library URLs from the verified copies in `assets`.
///
/// `work` is an empty directory that receives the served report and the browser profile.
///
/// # Errors
/// Returns copy, server, launch, asset, observation or cleanup failures.
pub async fn run(checkout: &Path, executable: &Path, assets: &Path, work: &Path) -> io::Result<()> {
    let root = work.join("report");
    tokio::fs::create_dir(&root).await?;
    tokio::fs::copy(
        checkout.join("crates/maestro-app/tests/fixtures/analysis/tool-report.html"),
        root.join("index.html"),
    )
    .await?;
    let profile = work.join("report-chromium");
    session(executable, &profile, &root, async |browser, origin| {
        page(browser, origin, assets).await
    })
    .await?;
    println!("PASS analysis_page");
    Ok(())
}

/// Observe the report only after the chart and style completion signals settle.
async fn page(browser: &mut Browser, origin: &str, directory: &Path) -> io::Result<()> {
    let mut page = browser.page("analysis_page").await?;
    let result = async {
        page.initialize().await?;
        page.cache = Some(assets::load(directory, origin).await?);
        page.execute("Fetch.enable", json!({"patterns":[{"urlPattern":"*"}]}))
            .await?;
        page.execute(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({"source":report_scripts::RENDERS}),
        )
        .await?;
        page.navigate(&format!("{origin}/")).await?;
        let mut observed = Observations::default();
        observed.compare(
            "cdn_assets_come_from_the_cache",
            &json!(escaped(&page)),
            &json!([]),
        );
        page.evaluate(
            "new Promise(done => requestAnimationFrame(() => requestAnimationFrame(done)))",
        )
        .await?;
        page.evaluate(report_scripts::PROBE).await?;
        histogram(&mut page, &mut observed).await?;
        security(&mut page, &mut observed).await?;
        colours(&mut page, &mut observed).await?;
        typography(&mut page, &mut observed).await?;
        // Page exceptions fail through the dispatcher before an assertion can complete.
        println!("PASS page_errors");
        observed.compare(
            "no_request_left_the_loopback_origin",
            &json!(escaped(&page)),
            &json!([]),
        );
        observed.finish()
    }
    .await;
    cleanup(result, page.close().await)
}

/// Every intercepted request that did not match the verified cache or local origin.
fn escaped<'a>(page: &'a Page<'_>) -> &'a [String] {
    page.cache
        .as_ref()
        .map_or(&[], |cache| cache.escaped.as_slice())
}

/// Row selection and explicit selection must paint the same changed histogram.
async fn histogram(page: &mut Page<'_>, observed: &mut Observations) -> io::Result<()> {
    page.evaluate("window.__settled('toolHistogram',0)").await?;
    let before = page
        .evaluate("({summary:probe.summary(),bitmap:probe.bitmap('#toolHistogram')})")
        .await?;
    let context = page.root_context()?;
    page.click(&context, "(() => { const node=document.querySelectorAll('#tools tbody tr[data-row]')[1];node.scrollIntoView({block:'center',inline:'center',behavior:'instant'});return node; })()").await?;
    page.evaluate("window.__settled('toolHistogram',1)").await?;
    let after = page
        .evaluate("({summary:probe.summary(),bitmap:probe.bitmap('#toolHistogram')})")
        .await?;
    page.evaluate("(() => { const select=document.querySelector('#toolSelect');select.value='1';select.dispatchEvent(new Event('input',{bubbles:true}));select.dispatchEvent(new Event('change',{bubbles:true})); })()").await?;
    page.evaluate("window.__settled('toolHistogram',2)").await?;
    let explicit = page.evaluate("probe.bitmap('#toolHistogram')").await?;
    observed.compare(
        "row_click_changes_the_summary",
        &json!(after["summary"] != before["summary"]),
        &json!(true),
    );
    observed.compare(
        "row_click_repaints_the_histogram",
        &json!(after["bitmap"] != before["bitmap"]),
        &json!(true),
    );
    observed.compare(
        "row_click_paints_the_selected_histogram",
        &json!(after["bitmap"] == explicit),
        &json!(true),
    );
    Ok(())
}

/// Authored payloads must remain text, never executable markup.
async fn security(page: &mut Page<'_>, observed: &mut Observations) -> io::Result<()> {
    observed.compare(
        "payload_stayed_inert",
        &page.evaluate("probe.payload()").await?,
        &json!(true),
    );
    observed.compare(
        "payload_name_stays_text",
        &page.evaluate("probe.payloadName()").await?,
        &json!("</script><script>globalThis.payloadExecuted=1</script>"),
    );
    observed.compare(
        "sessions_path_stays_text",
        &page.evaluate("probe.path()").await?,
        &json!({"text":"/fixture/sessions-<img src=x>","images":0}),
    );
    Ok(())
}

/// Count actual chart and screenshot pixels against the loaded theme's colour roles.
async fn colours(page: &mut Page<'_>, observed: &mut Observations) -> io::Result<()> {
    for (name, expression) in [
        (
            "bars_paint_the_accent",
            "probe.painted('#tokens',probe.roles.accent,{fromX:0.06,fromY:0.1,toY:0.6})>100",
        ),
        (
            "chart_text_paints_the_muted_colour",
            "probe.painted('#toolHistogram',probe.roles.muted,{fromY:0.75})>0",
        ),
        (
            "chart_grid_paints_the_border_colour",
            "probe.painted('#tokens',probe.roles.border,{fromY:0.15,toY:0.35})>100",
        ),
    ] {
        observed.compare(name, &page.evaluate(expression).await?, &json!(true));
    }
    for (name, selector, role) in [
        (
            "summary_text_paints_the_muted_colour",
            "#toolSummary",
            "muted",
        ),
        (
            "row_border_paints_the_border_colour",
            "#tools tbody tr[data-row]",
            "border",
        ),
        ("heading_paints_the_text_colour", "h1", "text"),
    ] {
        let colour: [u8; 3] =
            serde_json::from_value(page.evaluate(&format!("probe.roles.{role}")).await?)
                .map_err(io::Error::other)?;
        let screenshot = painted(page, selector).await?;
        observed.compare(
            name,
            &json!(counted(&screenshot.pixels, colour) > 0),
            &json!(true),
        );
    }
    Ok(())
}

/// Compare painted glyph extents with the declared shared font roles.
async fn typography(page: &mut Page<'_>, observed: &mut Observations) -> io::Result<()> {
    let title = page
        .evaluate("probe.chartTitle('#toolHistogram','estimated tokens/result')")
        .await?;
    let ink = number(&title, "ink")?;
    let declared = number(&title, "declared")?;
    observed.compare(
        "chart_title_paints_the_body_font",
        &json!((ink - declared).abs() <= 4.0),
        &json!(true),
    );
    let heading = painted(page, "h1").await?;
    let background: [u8; 3] =
        serde_json::from_value(page.evaluate("probe.roles.background").await?)
            .map_err(io::Error::other)?;
    let declared = page.evaluate("(() => {const role=getComputedStyle(document.documentElement).getPropertyValue('--maestro-font-display').trim();const style=getComputedStyle(document.querySelector('h1'));return probe.measure('h1',`${style.fontWeight} ${style.fontSize} ${role}`);})()").await?.as_f64().ok_or_else(|| io::Error::other("heading font measurement missing"))?;
    let ink = u32::try_from(ink_width(&heading.pixels, heading.width, background)?)
        .map_err(io::Error::other)?;
    observed.compare(
        "heading_paints_the_brand_font",
        &json!((f64::from(ink) / 1.25 - declared).abs() <= 8.0),
        &json!(true),
    );
    Ok(())
}

/// Decode a clipped real screenshot after scrolling the actual element into view.
async fn painted(page: &mut Page<'_>, selector: &str) -> io::Result<Painted> {
    let expression = format!(
        "(() => {{const node=document.querySelector({});if(!node) throw Error('screenshot element missing');node.scrollIntoView({{block:'center',inline:'center',behavior:'instant'}});const b=node.getBoundingClientRect();return {{x:b.x+scrollX,y:b.y+scrollY,width:b.width,height:b.height,scale:1}};}})()",
        json!(selector)
    );
    let clip = page.evaluate(&expression).await?;
    let decoded = page
        .screenshot(
            Some(clip),
            "({data, width}) => ({pixels: [...data], width})",
        )
        .await?;
    serde_json::from_value(decoded).map_err(io::Error::other)
}

#[cfg(test)]
mod tests;

/// Read one finite browser measurement without silently replacing a missing observation.
fn number(value: &Value, key: &str) -> io::Result<f64> {
    value[key]
        .as_f64()
        .filter(|number| number.is_finite())
        .ok_or_else(|| io::Error::other(format!("missing browser measurement {key}")))
}

/// Count opaque pixels of the exact supplied role colour.
fn counted(pixels: &[u8], colour: [u8; 3]) -> usize {
    pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[..3] == colour && pixel[3] == 255)
        .count()
}

/// Measure the horizontal extent of all ink against the known background.
fn ink_width(pixels: &[u8], width: usize, background: [u8; 3]) -> io::Result<usize> {
    if width == 0 || !pixels.len().is_multiple_of(4) {
        return Err(io::Error::other("invalid screenshot geometry"));
    }
    let mut left = width;
    let mut right = None;
    for (index, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
        if pixel[..3] != background {
            let column = index % width;
            left = left.min(column);
            right = Some(right.map_or(column, |right: usize| right.max(column)));
        }
    }
    Ok(right.map_or(0, |right| right - left + 1))
}
