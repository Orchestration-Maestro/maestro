//! Real screenshot observations answering the retained reading paint callback.
//!
//! Each authored scene names the painted state its export has just drawn. The driver captures
//! the actual page, mutates only the observed decoration, captures again and restores the exact
//! authored attributes, so a scene fails whenever the painted output differs from its contract.

use super::{
    page::{Page, string},
    paint_scripts,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::io;

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

/// Export selectors whose paint must reach the selected brand role inside the final clip.
const EXPORT_ROLES: [(&str, &str); 21] = [
    ("p", "text"),
    ("h1", "accent"),
    ("a", "accent"),
    ("p code", "panel"),
    ("blockquote", "border"),
    ("li", "text"),
    ("hr", "border"),
    ("th", "panel"),
    ("td", "border"),
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
            _ => match (scene.strip_prefix("viewport-"), neighbor(scene)) {
                (Some(index), _) => {
                    viewport(self, scene, index.parse().map_err(io::Error::other)?).await
                }
                (None, Some(selector)) => neighbors(self, scene, selector).await,
                (None, None) => Err(io::Error::other(format!("unknown paint scene {scene}"))),
            },
        }
    }

    /// Capture the whole visible page and keep its decoded pixels in the page under `label`.
    async fn shot(&mut self, label: &str) -> io::Result<()> {
        let png = self
            .execute("Page.captureScreenshot", json!({"format":"png"}))
            .await?;
        let source = format!("data:image/png;base64,{}", string(&png, "data")?);
        self.evaluate(&format!(
            "__maestroPaint.load({},{})",
            json!(label),
            json!(source)
        ))
        .await
        .map(drop)
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
    let selector = json!(selector);
    // A live selection highlight would paint over the observed text.
    page.evaluate("(() => {const s=getSelection();globalThis.__maestroSelection={anchor:s.anchorNode,anchorOffset:s.anchorOffset,focus:s.focusNode,focusOffset:s.focusOffset};s.removeAllRanges();})()").await?;
    if scene == "neighbors-artifact" {
        page.evaluate("Promise.all([...document.querySelectorAll('.maestro-pdf img')].map(i=>i.decode())).then(()=>null)").await?;
        page.shot("actual").await?;
        artifact_padding(page, scene, &selector).await?;
    } else {
        page.shot("actual").await?;
        text_paint(page, scene, &selector).await?;
    }
    let saved = page.evaluate(&format!("(() => {{const f=__maestroPaint.element({selector}).closest('.maestro-reading-frame'),s=f.getAttribute('style');f.style.boxShadow='none';return s;}})()")).await?;
    page.shot("without-frame-shadow").await?;
    page.evaluate(&format!("__maestroPaint.restyle([__maestroPaint.element({selector}).closest('.maestro-reading-frame')],[{saved}])")).await?;
    page.shot("restored").await?;
    let shadow: u64 = page
        .read("__maestroPaint.differ('without-frame-shadow','restored')")
        .await?;
    require(shadow == 0, "neighbors must gain no outer frame shadow")?;
    page.observed(scene, "no-frame-shadow", shadow);
    page.evaluate("(() => {const s=__maestroSelection;if(s.anchor)getSelection().setBaseAndExtent(s.anchor,s.anchorOffset,s.focus,s.focusOffset);delete globalThis.__maestroSelection;})()").await.map(drop)
}

/// Literal text inside the root paints: making it transparent changes pixels within the root.
async fn text_paint(page: &mut Page<'_>, scene: &str, selector: &Value) -> io::Result<()> {
    let bounds: Value = page
        .read(&format!("__maestroPaint.box({selector})"))
        .await?;
    page.evaluate(&format!("(() => {{const nodes=[...__maestroPaint.element({selector}).querySelectorAll('p,h1,h3')].flatMap(e=>[e,...e.querySelectorAll('*')]);globalThis.__maestroText=[nodes,nodes.map(n=>n.getAttribute('style'))];for(const n of nodes) n.style.setProperty('color','transparent','important');}})()")).await?;
    page.shot("without-text").await?;
    page.evaluate(
        "(() => {__maestroPaint.restyle(...__maestroText);delete globalThis.__maestroText;})()",
    )
    .await?;
    let painted: u64 = page
        .read(&format!(
            "__maestroPaint.differ('actual','without-text',{bounds})"
        ))
        .await?;
    require(painted > 0, "literal native text must actually paint")?;
    page.observed(scene, "text", painted);
    Ok(())
}

/// Artifact padding paints the caller's background role and the page-local shadows.
async fn artifact_padding(page: &mut Page<'_>, scene: &str, selector: &Value) -> io::Result<()> {
    let sample: Value = page.read(&format!("(() => {{const r=__maestroPaint.element({selector}),b=r.getBoundingClientRect(),c=document.createElement('canvas').getContext('2d');c.fillStyle=getComputedStyle(r).getPropertyValue('--maestro-color-background');c.fillRect(0,0,1,1);return {{point:[b.x+4,b.y+4],colour:[...c.getImageData(0,0,1,1).data]}};}})()")).await?;
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
    page.evaluate("(() => {const nodes=[...document.querySelectorAll('.maestro-pdf img')];globalThis.__maestroPages=[nodes,nodes.map(n=>n.getAttribute('style'))];for(const n of nodes) n.style.boxShadow='none';})()").await?;
    page.shot("pages-without-shadow").await?;
    page.evaluate(
        "(() => {__maestroPaint.restyle(...__maestroPages);delete globalThis.__maestroPages;})()",
    )
    .await?;
    let shadows: u64 = page
        .read("__maestroPaint.differ('actual','pages-without-shadow')")
        .await?;
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
    let counts: Value = page
        .read(&format!(
            "__maestroPaint.viewport({},{mapping})",
            json!(scene)
        ))
        .await?;
    let count = |key: &str| counts[key].as_u64().unwrap_or(u64::MAX);
    require(
        count("green") == 0,
        "absolute sentinel must be clipped by the viewport",
    )?;
    require(
        count("escaped") == 0,
        "ancestor and viewport clips must constrain real paint",
    )?;
    require(count("holes") == 0, "control holes must remove real paint")?;
    if (4..=7).contains(&index) {
        require(count("red") == 0, "an empty viewport must paint no content")?;
        require(
            count("magenta") == 0,
            "empty viewport must have no residual shadow",
        )?;
    } else {
        require(count("red") > 0, "the visible viewport must paint content")?;
        require(
            (count("magenta") > 0) == (index != 2),
            "finite ancestor margin must retain allowed outer decoration",
        )?;
    }
    let hits: u64 = page.read(&format!("(({{left,top,scale,viewport,clip,holes}}) => {{const r=document.querySelector('.test-slot').parentElement.parentElement,inside=(q,b)=>q[0]>b[0]&&q[0]<b[2]&&q[1]>b[1]&&q[1]<b[3];let n=0;for(let y=82.5;y<200;y+=5)for(let x=82.5;x<440;x+=5){{const e=document.elementFromPoint(left+x*scale[0],top+y*scale[1]);const allowed=inside([x,y],viewport)&&inside([x,y],clip)&&!holes.some(h=>inside([x,y],h));if(e&&r.contains(e)&&!allowed)n++}}return n}})({mapping})")).await?;
    require(
        hits == 0,
        "native hits must not escape clips or enter canvas control holes",
    )?;
    page.observed(scene, "content", count("red"));
    page.observed(scene, "decoration", count("magenta"));
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
    let geometry: Value = page.read("(() => {const c=document.getElementById('canvas'),b=c.getBoundingClientRect(),d=devicePixelRatio,root=document.querySelector('.test-slot').parentElement.parentElement,v=root.parentElement.getBoundingClientRect(),r=root.getBoundingClientRect();return {left:b.left,top:b.top,scale:[b.width*d/c.width,b.height*d/c.height],canvas:[0,0,c.width/d,c.height/d],native:[v.x,v.y,v.width,v.height,r.width]};})()").await?;
    let number = |value: &Value| value.as_f64().unwrap_or(f64::NAN);
    let (left, top) = (number(&geometry["left"]), number(&geometry["top"]));
    let scale = [number(&geometry["scale"][0]), number(&geometry["scale"][1])];
    let expected = [
        left + origin[0] * scale[0],
        top + origin[1] * scale[1],
        size[0] * scale[0],
        size[1] * scale[1],
        700.0 * scale[0],
    ];
    require(
        expected
            .iter()
            .enumerate()
            .all(|(i, value)| (number(&geometry["native"][i]) - value).abs() < 0.02),
        "clipping must preserve the full viewport and feature layout, not resize to the visible intersection",
    )?;
    let canvas = &geometry["canvas"];
    let clip = CLIPS
        .get(index)
        .copied()
        .flatten()
        .map_or_else(|| canvas.clone(), |clip| json!(clip));
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

/// Count `colour` pixels of the first `selector` match within its final export clip.
async fn role_paint(
    page: &mut Page<'_>,
    scene: &str,
    selector: &str,
    colour: &str,
) -> io::Result<()> {
    let element = format!(
        "document.querySelector('.maestro-export').querySelector({})",
        json!(selector)
    );
    page.evaluate(&format!(
        "__maestroPaint.element({element}).scrollIntoView({{block:'nearest',inline:'nearest'}})"
    ))
    .await?;
    page.park().await?;
    let rect: Value = page.read(&format!("(() => {{const e=__maestroPaint.element({element}),b=e.getBoundingClientRect(),c=e.closest('.maestro-export').parentElement.getBoundingClientRect();return b.width>0&&b.height>0?[Math.max(b.left,c.left,0),Math.max(b.top,c.top,0),Math.min(b.right,c.right,innerWidth),Math.min(b.bottom,c.bottom,innerHeight)]:null;}})()")).await?;
    require(!rect.is_null(), &format!("{selector} has geometry"))?;
    page.shot(selector).await?;
    let pixels: u64 = page
        .read(&format!(
            "__maestroPaint.count({},{rect},{colour})",
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

/// Count differing pixels of two captures within the current box of `element`.
async fn changed(
    page: &mut Page<'_>,
    element: &str,
    (first, second): (&str, &str),
) -> io::Result<u64> {
    page.read(&format!(
        "__maestroPaint.differ({},{},__maestroPaint.box({element}))",
        json!(first),
        json!(second)
    ))
    .await
}

/// Capture, restyle `element` with `css`, capture again and restore its authored style.
async fn without(page: &mut Page<'_>, element: &str, css: &str, label: &str) -> io::Result<u64> {
    page.shot("decorated").await?;
    page.evaluate(&format!("(() => {{const e=__maestroPaint.element({element});globalThis.__maestroSaved=[[e],[e.getAttribute('style')]];e.style.cssText+={};}})()", json!(css))).await?;
    page.shot(label).await?;
    page.evaluate(
        "(() => {__maestroPaint.restyle(...__maestroSaved);delete globalThis.__maestroSaved;})()",
    )
    .await?;
    changed(page, element, ("decorated", label)).await
}

/// Hovering an expandable output changes its painted opacity.
async fn output_hover(page: &mut Page<'_>, scene: &str, element: &str) -> io::Result<()> {
    page.evaluate(&format!(
        "__maestroPaint.element({element}).scrollIntoView({{block:'nearest',inline:'nearest'}})"
    ))
    .await?;
    page.park().await?;
    page.shot("before-hover").await?;
    page.hover(element).await?;
    page.shot("hovered").await?;
    let hovered = changed(page, element, ("before-hover", "hovered")).await?;
    require(hovered > 0, "output hover paints opacity")?;
    page.observed(scene, "output-hover", hovered);
    Ok(())
}

/// Every selected export role paints, and disclosure, hover and underline change real pixels.
async fn export_roles(page: &mut Page<'_>) -> io::Result<()> {
    let scene = "export-roles";
    for (selector, role) in EXPORT_ROLES {
        let colour = format!("__maestroPaint.role({})", json!(role));
        if matches!(selector, "hr" | "td") {
            border_role(page, selector, role).await?;
        } else {
            role_paint(page, scene, selector, &colour).await?;
        }
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
    let expanded = changed(page, output, ("before-hover", "expanded")).await?;
    require(expanded > 0, "expanded body paints distinct text")?;
    page.observed(scene, "expanded", expanded);
    let ansi = "document.querySelector('.maestro-export .ansi-line span')";
    let underline = without(page, ansi, ";text-decoration:none", "unlined").await?;
    require(underline > 0, "supplied underline paints")?;
    page.observed(scene, "underline", underline);
    Ok(())
}

/// A border paints exactly the selected role and contributes pixels of its own.
async fn border_role(page: &mut Page<'_>, selector: &str, role: &str) -> io::Result<()> {
    let element = format!(
        "document.querySelector('.maestro-export').querySelector({})",
        json!(selector)
    );
    page.evaluate(&format!(
        "__maestroPaint.element({element}).scrollIntoView({{block:'nearest',inline:'nearest'}})"
    ))
    .await?;
    page.park().await?;
    page.shot("border").await?;
    page.evaluate(&format!("(() => {{const e=__maestroPaint.element({element});globalThis.__maestroSaved=[[e],[e.getAttribute('style')]];e.style.borderColor=JSON.parse(globalThis.colors)[{}];}})()", json!(role))).await?;
    page.shot("reference").await?;
    page.evaluate(&format!(
        "__maestroPaint.element({element}).style.borderColor='transparent'"
    ))
    .await?;
    page.shot("absent").await?;
    page.evaluate(
        "(() => {__maestroPaint.restyle(...__maestroSaved);delete globalThis.__maestroSaved;})()",
    )
    .await?;
    let differs: u64 = page
        .read("__maestroPaint.differ('border','reference')")
        .await?;
    require(
        differs == 0,
        &format!("{selector} matches selected border paint"),
    )?;
    let contributes: u64 = page
        .read("__maestroPaint.differ('border','absent')")
        .await?;
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
    role_paint(page, scene, ".tool-image", "[0,0,0]").await?;
    let diff = "document.querySelector('.maestro-export .tool-diff')";
    page.evaluate(&format!(
        "__maestroPaint.element({diff}).scrollIntoView({{block:'nearest',inline:'nearest'}})"
    ))
    .await?;
    page.shot("diff").await?;
    page.evaluate(&format!("(() => {{const d=__maestroPaint.element({diff}),nodes=[d,...d.querySelectorAll('div')];globalThis.__maestroSaved=[nodes,nodes.map(n=>n.getAttribute('style'))];for(const n of nodes) n.style.color='transparent';}})()")).await?;
    page.shot("without-diff").await?;
    page.evaluate(
        "(() => {__maestroPaint.restyle(...__maestroSaved);delete globalThis.__maestroSaved;})()",
    )
    .await?;
    let painted: u64 = page
        .read("__maestroPaint.differ('diff','without-diff')")
        .await?;
    require(painted > 0, "diff text paints")?;
    page.observed(scene, "diff", painted);
    Ok(())
}

/// Tool-card roles paint, output hover changes opacity and supplied spaces keep decoration.
async fn tool_card_roles(page: &mut Page<'_>) -> io::Result<()> {
    let scene = "offline-tool-card-roles";
    for (selector, role) in TOOL_CARD_ROLES {
        let colour = format!("__maestroPaint.role({})", json!(role));
        role_paint(page, scene, selector, &colour).await?;
    }
    let output = "[...document.querySelectorAll('.maestro-export .tool-output.expandable')].at(-1)";
    output_hover(page, scene, output).await?;
    let custom = ".ansi-rendered span";
    role_paint(page, scene, custom, "[18,52,86]").await?;
    let custom = "document.querySelector('.maestro-export .ansi-rendered span')";
    let decoration = without(
        page,
        custom,
        ";background:transparent;text-decoration:none",
        "suppressed",
    )
    .await?;
    require(decoration > 0, "supplied spaces contribute decoration")?;
    page.observed(scene, "space-decoration", decoration);
    let underline = without(page, custom, ";text-decoration:none", "unlined").await?;
    require(underline > 0, "underline on spaces paints")?;
    page.observed(scene, "space-underline", underline);
    Ok(())
}
