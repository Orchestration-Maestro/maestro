//! Real screenshot observations answering the retained reading paint callback.
//!
//! Each authored scene names the painted state its export has just drawn. The driver captures
//! the actual page, mutates only the observed decoration, captures again and restores the exact
//! authored attributes, so a scene fails whenever the painted output differs from its contract.

use super::{
    page::{Page, string},
    paint_scripts,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::io;

/// Artifact console scenes.
mod console;
/// HTML artifact source listing scenes.
mod html;
/// Visible boxes and their pixel counts.
mod region;
/// Word page scenes.
mod word;

/// A page point far from every mounted fixture, used to clear native hover.
const PARKED: (f64, f64) = (1279.0, 719.0);

/// Canvas-control holes the viewport export subtracts, in canvas points.
const HOLES: [[f64; 4]; 3] = [
    [200.0, 125.0, 240.0, 150.0],
    [90.0, 170.0, 120.0, 200.0],
    [600.0, 400.0, 650.0, 450.0],
];

/// Bounded clips of the viewport export's first six placements, in canvas points.
const CLIPS: [Option<[f64; 4]>; 6] = [
    None,
    Some([70.0, 70.0, 500.0, 300.0]),
    Some([100.0, 100.0, 400.0, 180.0]),
    Some([150.0, 110.0, 500.0, 300.0]),
    Some([0.0, 0.0, 0.0, 0.0]),
    Some([700.0, 400.0, 800.0, 500.0]),
];

/// The last authored viewport placement the export observes.
const LAST_VIEWPORT: usize = 8;

/// Width of the accent ring a focused control draws inside its edges, in CSS pixels.
const RING: f64 = 2.0;

/// Export selectors whose paint must reach the selected brand role inside the final clip.
const EXPORT_ROLES: [(&str, &str); 19] = [
    ("p", "text"),
    ("h1", "accent"),
    ("a", "accent"),
    ("p code", "panel"),
    ("blockquote", "border"),
    ("li", "text"),
    ("th", "panel"),
    (".hljs-comment", "muted"),
    (".hljs-keyword", "accent"),
    (".hljs-function", "accent"),
    (".hljs-variable", "text"),
    (".hljs-string", "success"),
    (".hljs-number", "warning"),
    (".hljs-type", "live"),
    (".hljs-operator", "text"),
    (".hljs-punctuation", "text"),
    (".output-preview", "muted"),
    (".expand-hint", "muted"),
    (".ansi-line span", "accent"),
];

/// Export selectors whose border must paint exactly the selected brand role.
const EXPORT_BORDERS: [(&str, &str); 2] = [("hr", "border"), ("td", "border")];

/// Tool-card selectors whose paint must reach the selected brand role inside the final clip.
const TOOL_CARD_ROLES: [(&str, &str); 13] = [
    (".pending", "panel"),
    (".success", "panel"),
    (".error", "panel"),
    (".tool-command", "text"),
    (".tool-name", "text"),
    (".tool-path", "accent"),
    (".line-numbers", "warning"),
    (".line-count", "muted"),
    (".tool-error", "error"),
    (".tool-output", "muted"),
    (".diff-added", "success"),
    (".diff-removed", "error"),
    (".diff-context", "muted"),
];

/// Classified pixel counts of one viewport capture.
#[derive(Deserialize)]
struct Counts {
    /// Pure red content pixels.
    red: u64,
    /// Pure green sentinel pixels.
    green: u64,
    /// Pure magenta outer-decoration pixels.
    magenta: u64,
    /// Content or decoration pixels beyond the viewport, clip or canvas.
    escaped: u64,
    /// Content or decoration pixels inside a canvas-control hole.
    holes: u64,
}

/// Live canvas and native viewport geometry, in page CSS pixels.
#[derive(Deserialize)]
struct Geometry {
    /// Canvas left edge.
    left: f64,
    /// Canvas top edge.
    top: f64,
    /// Page pixels per canvas point, horizontally and vertically.
    scale: [f64; 2],
    /// Canvas bounds in canvas points.
    canvas: [f64; 4],
    /// Native viewport position and size, then the reading root width.
    native: [f64; 5],
}

impl Page<'_> {
    /// Observe one authored scene, failing with the first painted difference from its contract.
    ///
    /// # Errors
    /// Returns capture, decoding and protocol failures, an unknown scene or a failed observation.
    pub(super) async fn reading_paint(&mut self, scene: &str) -> io::Result<()> {
        self.evaluate(paint_scripts::PAINT).await?;
        match scene {
            "link-hover" => {
                self.hover("document.querySelector('.test-slot').parentElement.querySelector('p')")
                    .await
            }
            "export-roles" => export_roles(self).await,
            "offline-tool-card-layout" => tool_card_layout(self).await,
            "offline-tool-card-roles" => tool_card_roles(self).await,
            "control-focus" => control_focus(self).await,
            "console-rest" | "console-hover" | "console-focus" | "console-collapsed"
            | "console-expanded" | "console-copy" | "console-copied" => {
                console::observe(self, scene).await
            }
            "html-source" => html::shown(self).await,
            "html-source-again" => html::again(self).await,
            "word-page" => word::sheet(self).await,
            _ => placed(self, scene).await,
        }
    }

    /// Capture the visible page, or only `clip`, decode it in the page and return what the page
    /// function `consumer` makes of the decoded image data.
    ///
    /// # Errors
    /// Returns capture, decoding and protocol failures.
    pub(super) async fn screenshot(
        &mut self,
        clip: Option<Value>,
        consumer: &str,
    ) -> io::Result<Value> {
        let mut params = json!({"format":"png"});
        if let Some(clip) = clip {
            params["clip"] = clip;
        }
        let png = self.execute("Page.captureScreenshot", params).await?;
        let source = format!("data:image/png;base64,{}", string(&png, "data")?);
        self.evaluate(&format!(
            "({})({}).then({consumer})",
            paint_scripts::DECODE,
            json!(source)
        ))
        .await
    }

    /// Capture the whole visible page and keep its decoded pixels in the page under `label`.
    async fn shot(&mut self, label: &str) -> io::Result<()> {
        let consumer = format!("shot => __maestroPaint.store({}, shot)", json!(label));
        self.screenshot(None, &consumer).await.map(drop)
    }

    /// Evaluate a page expression and read its JSON value as `T`.
    async fn read<T: DeserializeOwned>(&mut self, expression: &str) -> io::Result<T> {
        serde_json::from_value(self.evaluate(expression).await?).map_err(io::Error::other)
    }

    /// Move the native pointer away from every fixture, so no hover paint remains.
    async fn park(&mut self) -> io::Result<()> {
        let root = self.root_context()?;
        self.pointer(&root, PARKED, false).await
    }

    /// Record one passed observation with its measured pixel count.
    fn observed(&mut self, scene: &str, check: &str, pixels: u64) {
        self.log(&format!(
            "PAINT {} {scene} {check} pixels={pixels}",
            self.name
        ));
    }
}

/// Observe a numbered viewport scene or a neighbour scene.
async fn placed(page: &mut Page<'_>, scene: &str) -> io::Result<()> {
    match (scene.strip_prefix("viewport-"), neighbor(scene)) {
        (Some(index), _) => {
            let index = index
                .parse()
                .ok()
                .filter(|index| *index <= LAST_VIEWPORT)
                .ok_or_else(|| io::Error::other(format!("unknown paint scene {scene}")))?;
            viewport(page, scene, index).await
        }
        (None, Some(selector)) => neighbors(page, scene, selector).await,
        (None, None) => Err(io::Error::other(format!("unknown paint scene {scene}"))),
    }
}

/// The reading root each neighbour scene paints.
fn neighbor(scene: &str) -> Option<&'static str> {
    match scene {
        "neighbors-artifact" => Some(".maestro-pdf-artifact"),
        "neighbors-error" => Some(".maestro-file-error"),
        "neighbors-generic" => Some(".maestro-generic-artifact"),
        "neighbors-header" => Some(".maestro-session-header"),
        _ => None,
    }
}

/// Fail with `message` unless `condition` holds.
fn require(condition: bool, message: &str) -> io::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message.to_owned()))
    }
}

/// Neighbouring reading roots keep their own paint and gain no outer frame shadow.
async fn neighbors(page: &mut Page<'_>, scene: &str, selector: &str) -> io::Result<()> {
    let root = format!("__maestroPaint.element({})", json!(selector));
    // A live selection highlight would paint over the observed text.
    page.evaluate("(() => {const s=getSelection();globalThis.__maestroSelection={anchor:s.anchorNode,anchorOffset:s.anchorOffset,focus:s.focusNode,focusOffset:s.focusOffset};s.removeAllRanges();})()").await?;
    if scene == "neighbors-artifact" {
        artifact_padding(page, scene, &root).await?;
    } else {
        let text = format!(
            "[...{root}.querySelectorAll('p,h1,h3')].flatMap(e=>[e,...e.querySelectorAll('*')])"
        );
        let painted = without(
            page,
            &text,
            "n=>n.style.setProperty('color','transparent','important')",
            Some(&root),
        )
        .await?;
        require(painted > 0, "literal native text must actually paint")?;
        page.observed(scene, "text", painted);
    }
    let frame = format!("[{root}.closest('.maestro-reading-frame')]");
    let shadow = without(page, &frame, "n=>n.style.boxShadow='none'", None).await?;
    require(shadow == 0, "neighbors must gain no outer frame shadow")?;
    page.observed(scene, "no-frame-shadow", shadow);
    page.evaluate("(() => {const s=__maestroSelection;if(s.anchor)getSelection().setBaseAndExtent(s.anchor,s.anchorOffset,s.focus,s.focusOffset);delete globalThis.__maestroSelection;})()").await.map(drop)
}

/// Artifact padding paints the caller's background role and the page-local shadows.
async fn artifact_padding(page: &mut Page<'_>, scene: &str, root: &str) -> io::Result<()> {
    let pages = "document.querySelectorAll('.maestro-pdf img')";
    page.evaluate(&format!(
        "Promise.all([...{pages}].map(i=>i.decode())).then(()=>null)"
    ))
    .await?;
    page.shot("actual").await?;
    let sample: Value = page.read(&format!("(() => {{const r={root},b=r.getBoundingClientRect(),c=document.createElement('canvas').getContext('2d');c.fillStyle=getComputedStyle(r).getPropertyValue('--maestro-color-background');c.fillRect(0,0,1,1);return {{point:[b.x+4,b.y+4],colour:[...c.getImageData(0,0,1,1).data]}};}})()")).await?;
    let pixel: Value = page
        .read(&format!(
            "__maestroPaint.pixel('actual',{})",
            sample["point"]
        ))
        .await?;
    require(
        pixel == sample["colour"],
        "artifact padding must paint the caller background role",
    )?;
    let shadows = without(page, pages, "n=>n.style.boxShadow='none'", None).await?;
    require(
        shadows > 0,
        "page-local shadows must actually paint inside artifact padding",
    )?;
    page.observed(scene, "page-shadows", shadows);
    Ok(())
}

/// Content, sentinel and outer decoration stay inside the viewport, ancestor clip and holes.
async fn viewport(page: &mut Page<'_>, scene: &str, index: usize) -> io::Result<()> {
    let mapping = placement(page, index).await?;
    page.shot(scene).await?;
    let counts: Counts = page
        .read(&format!(
            "__maestroPaint.viewport({},{mapping})",
            json!(scene)
        ))
        .await?;
    require(
        counts.green == 0,
        "absolute sentinel must be clipped by the viewport",
    )?;
    require(
        counts.escaped == 0,
        "ancestor and viewport clips must constrain real paint",
    )?;
    require(counts.holes == 0, "control holes must remove real paint")?;
    if (4..=7).contains(&index) {
        require(counts.red == 0, "an empty viewport must paint no content")?;
        require(
            counts.magenta == 0,
            "empty viewport must have no residual shadow",
        )?;
    } else {
        require(counts.red > 0, "the visible viewport must paint content")?;
        require(
            (counts.magenta > 0) == (index != 2),
            "finite ancestor margin must retain allowed outer decoration",
        )?;
    }
    let hits: u64 = page.read(&format!("(({{left,top,scale,viewport,clip,holes}}) => {{const r=document.querySelector('.test-slot').parentElement.parentElement,inside=(q,b)=>q[0]>b[0]&&q[0]<b[2]&&q[1]>b[1]&&q[1]<b[3];let n=0;for(let y=82.5;y<200;y+=5)for(let x=82.5;x<440;x+=5){{const e=document.elementFromPoint(left+x*scale[0],top+y*scale[1]);const allowed=inside([x,y],viewport)&&inside([x,y],clip)&&!holes.some(h=>inside([x,y],h));if(e&&r.contains(e)&&!allowed)n++}}return n}})({mapping})")).await?;
    require(
        hits == 0,
        "native hits must not escape clips or enter canvas control holes",
    )?;
    page.observed(scene, "content", counts.red);
    page.observed(scene, "decoration", counts.magenta);
    Ok(())
}

/// Require the native viewport at its full authored placement, then map canvas points to the page.
async fn placement(page: &mut Page<'_>, index: usize) -> io::Result<Value> {
    let origin = if index == 8 {
        [-20.5, 80.25]
    } else {
        [100.0, 100.0]
    };
    let size = match index {
        6 => [0.0, 80.0],
        7 => [300.0, 0.0],
        _ => [300.0, 80.0],
    };
    let geometry: Geometry = page.read("(() => {const c=document.getElementById('canvas'),b=c.getBoundingClientRect(),d=devicePixelRatio,root=document.querySelector('.test-slot').parentElement.parentElement,v=root.parentElement.getBoundingClientRect(),r=root.getBoundingClientRect();return {left:b.left,top:b.top,scale:[b.width*d/c.width,b.height*d/c.height],canvas:[0,0,c.width/d,c.height/d],native:[v.x,v.y,v.width,v.height,r.width]};})()").await?;
    let Geometry {
        left,
        top,
        scale,
        canvas,
        native,
    } = geometry;
    let expected = [
        left + origin[0] * scale[0],
        top + origin[1] * scale[1],
        size[0] * scale[0],
        size[1] * scale[1],
        700.0 * scale[0],
    ];
    require(
        native
            .iter()
            .zip(expected)
            .all(|(actual, value)| (actual - value).abs() < 0.02),
        "clipping must preserve the full viewport and feature layout, not resize to the visible intersection",
    )?;
    let clip = CLIPS.get(index).copied().flatten().unwrap_or(canvas);
    let viewport = [
        origin[0],
        origin[1],
        origin[0] + size[0],
        origin[1] + size[1],
    ];
    Ok(
        json!({"left":left,"top":top,"scale":scale,"viewport":viewport,"clip":clip,"canvas":canvas,"holes":HOLES}),
    )
}

/// Scroll `element` into view and park the pointer, so a capture shows it without hover paint.
async fn reveal(page: &mut Page<'_>, element: &str) -> io::Result<()> {
    page.evaluate(&format!(
        "__maestroPaint.element({element}).scrollIntoView({{block:'nearest',inline:'nearest'}})"
    ))
    .await?;
    page.park().await
}

/// The first `selector` match inside the export, as a page expression.
fn exported(selector: &str) -> String {
    format!(
        "document.querySelector('.maestro-export').querySelector({})",
        json!(selector)
    )
}

/// Count pixels of `colour`, a brand role name or literal RGB channels, painted by the first
/// `selector` match within its final export clip.
async fn role_paint(
    page: &mut Page<'_>,
    scene: &str,
    selector: &str,
    colour: &Value,
) -> io::Result<()> {
    let element = exported(selector);
    reveal(page, &element).await?;
    let rect: Value = page.read(&format!("(() => {{const e=__maestroPaint.element({element}),b=e.getBoundingClientRect(),c=e.closest('.maestro-export').parentElement.getBoundingClientRect();return b.width>0&&b.height>0?[Math.max(b.left,c.left,0),Math.max(b.top,c.top,0),Math.min(b.right,c.right,innerWidth),Math.min(b.bottom,c.bottom,innerHeight)]:null;}})()")).await?;
    require(!rect.is_null(), &format!("{selector} has geometry"))?;
    page.shot(selector).await?;
    let pixels: u64 = page
        .read(&format!(
            "__maestroPaint.count({},{rect},__maestroPaint.colour({colour}))",
            json!(selector)
        ))
        .await?;
    require(
        pixels > 0,
        &format!("{selector} paints {colour} inside the final clip"),
    )?;
    page.observed(scene, selector, pixels);
    Ok(())
}

/// Count differing pixels of two captures, within the current box of `region` when given and
/// across the whole page otherwise.
async fn changed(
    page: &mut Page<'_>,
    region: Option<&str>,
    (first, second): (&str, &str),
) -> io::Result<u64> {
    let rect = region.map_or_else(
        || "null".to_owned(),
        |element| format!("__maestroPaint.box({element})"),
    );
    page.read(&format!(
        "__maestroPaint.differ({},{},{rect})",
        json!(first),
        json!(second)
    ))
    .await
}

/// Capture as `decorated`, apply the page function `change` to every node of `nodes`, capture
/// again as `without` and restore their authored `style` attributes.
async fn contrast(page: &mut Page<'_>, nodes: &str, change: &str) -> io::Result<()> {
    page.shot("decorated").await?;
    page.evaluate(&format!("__maestroPaint.mutate([...{nodes}],{change})"))
        .await?;
    page.shot("without").await?;
    page.evaluate("__maestroPaint.restore()").await.map(drop)
}

/// Contrast `nodes` with and without `change` and count the changed pixels, as `changed` does
/// for `region`.
async fn without(
    page: &mut Page<'_>,
    nodes: &str,
    change: &str,
    region: Option<&str>,
) -> io::Result<u64> {
    contrast(page, nodes, change).await?;
    changed(page, region, ("decorated", "without")).await
}

/// Hovering an expandable output changes its painted opacity.
async fn output_hover(page: &mut Page<'_>, scene: &str, element: &str) -> io::Result<()> {
    reveal(page, element).await?;
    page.shot("before-hover").await?;
    page.hover(element).await?;
    page.shot("hovered").await?;
    let hovered = changed(page, Some(element), ("before-hover", "hovered")).await?;
    require(hovered > 0, "output hover paints opacity")?;
    page.observed(scene, "output-hover", hovered);
    Ok(())
}

/// Every selected export role paints, and disclosure, hover and underline change real pixels.
async fn export_roles(page: &mut Page<'_>) -> io::Result<()> {
    let scene = "export-roles";
    for (selector, role) in EXPORT_ROLES {
        role_paint(page, scene, selector, &json!(role)).await?;
    }
    for (selector, role) in EXPORT_BORDERS {
        border_role(page, selector, role).await?;
    }
    let output = "document.querySelector('.maestro-export .tool-output.expandable')";
    output_hover(page, scene, output).await?;
    let root = page.root_context()?;
    page.click(&root, output).await?;
    require(
        page.read::<bool>("(() => {const o=document.querySelector('.maestro-export .tool-output.expandable');return !o.querySelector('.output-preview').checkVisibility()&&o.querySelector('.output-full').checkVisibility();})()").await?,
        "a click expands the output",
    )?;
    page.park().await?;
    page.shot("expanded").await?;
    let expanded = changed(page, Some(output), ("before-hover", "expanded")).await?;
    require(expanded > 0, "expanded body paints distinct text")?;
    page.observed(scene, "expanded", expanded);
    let ansi = "document.querySelector('.maestro-export .ansi-line span')";
    reveal(page, ansi).await?;
    let underline = without(
        page,
        &format!("[{ansi}]"),
        "n=>n.style.textDecoration='none'",
        Some(ansi),
    )
    .await?;
    require(underline > 0, "supplied underline paints")?;
    page.observed(scene, "underline", underline);
    Ok(())
}

/// A border paints exactly the selected role and contributes pixels of its own.
async fn border_role(page: &mut Page<'_>, selector: &str, role: &str) -> io::Result<()> {
    let element = exported(selector);
    reveal(page, &element).await?;
    let nodes = format!("[{element}]");
    let reference = format!(
        "n=>n.style.borderColor=JSON.parse(globalThis.colors)[{}]",
        json!(role)
    );
    let differs = without(page, &nodes, &reference, None).await?;
    require(
        differs == 0,
        &format!("{selector} matches selected border paint"),
    )?;
    let transparent = "n=>n.style.borderColor='transparent'";
    let contributes = without(page, &nodes, transparent, None).await?;
    require(
        contributes > 0,
        &format!("{selector} border contributes pixels"),
    )?;
    page.observed("export-roles", selector, contributes);
    Ok(())
}

/// The decoded image and the unwrapped diff text paint real pixels.
async fn tool_card_layout(page: &mut Page<'_>) -> io::Result<()> {
    let scene = "offline-tool-card-layout";
    role_paint(page, scene, ".tool-image", &json!([0, 0, 0])).await?;
    let diff = "__maestroPaint.element(document.querySelector('.maestro-export .tool-diff'))";
    reveal(page, diff).await?;
    let text = format!("[{diff},...{diff}.querySelectorAll('div')]");
    let painted = without(page, &text, "n=>n.style.color='transparent'", None).await?;
    require(painted > 0, "diff text paints")?;
    page.observed(scene, "diff", painted);
    Ok(())
}

/// Tool-card roles paint, output hover changes opacity and supplied spaces keep their background.
async fn tool_card_roles(page: &mut Page<'_>) -> io::Result<()> {
    let scene = "offline-tool-card-roles";
    for (selector, role) in TOOL_CARD_ROLES {
        role_paint(page, scene, selector, &json!(role)).await?;
    }
    let output = "[...document.querySelectorAll('.maestro-export .tool-output.expandable')].at(-1)";
    output_hover(page, scene, output).await?;
    let custom = ".ansi-rendered span";
    role_paint(page, scene, custom, &json!([18, 52, 86])).await?;
    let custom = "document.querySelector('.maestro-export .ansi-rendered span')";
    let background = without(
        page,
        &format!("[{custom}]"),
        "n=>{n.style.background='transparent';n.style.textDecoration='none'}",
        Some(custom),
    )
    .await?;
    // The browser skips decoration lines on leading and trailing spaces
    // (`text-decoration-skip-spaces: start end`), so an all-space run paints only its background.
    require(background > 0, "supplied spaces keep their background")?;
    page.observed(scene, "space-background", background);
    Ok(())
}

/// The focused control's accent ring covers more than half of each edge band, so no ancestor
/// clips it away on any side.
async fn control_focus(page: &mut Page<'_>) -> io::Result<()> {
    let scene = "control-focus";
    let focused = "document.activeElement";
    require(
        page.read(&format!("{focused}!==document.body")).await?,
        "a control must hold focus",
    )?;
    reveal(page, focused).await?;
    page.shot(scene).await?;
    let edges: [[u64; 2]; 4] = page.read(&format!("(() => {{const [l,t,r,b]=__maestroPaint.box({focused}),w={RING},d=devicePixelRatio,accent=__maestroPaint.colour('accent');return [[l,t,r,t+w],[r-w,t,r,b],[l,b-w,r,b],[l,t,l+w,b]].map(e=>[__maestroPaint.count({},e,accent),Math.round((e[2]-e[0])*(e[3]-e[1])*d*d)]);}})()", json!(scene))).await?;
    for (edge, [pixels, area]) in ["top", "right", "bottom", "left"].into_iter().zip(edges) {
        require(
            pixels * 2 > area,
            &format!("the focus ring paints accent along the {edge} edge"),
        )?;
        page.observed(scene, edge, pixels);
    }
    Ok(())
}
