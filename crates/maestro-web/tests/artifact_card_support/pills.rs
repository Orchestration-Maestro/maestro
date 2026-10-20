//! Panel selection and nested disclosure admission through real native filenames.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{
    markdown_support::{Surface, check, js, publish},
    support::{reading_view::view, tool_output::TextOnlyOutput as _},
    tool_renderer_browser as drawing,
};
use maestro_app::browser::tools::artifacts::LocalArtifactsPanel;
use maestro_web::{
    ArtifactsToolRenderer, ToolRenderer,
    tools::types::{ToolRenderResult, ToolRenderState},
};
use serde_json::{Value, json};
use std::rc::Rc;

/// A native card and its real application panel, with controlled frame input.
struct Card {
    surface: Surface,
    recipe: ToolRenderResult,
    state: ToolRenderState,
    panel: Rc<LocalArtifactsPanel>,
    presentation: Rc<crate::presentation::Presentation>,
    enabled: bool,
    clip: Option<egui::Rect>,
}
impl Card {
    fn new(filename: &Value, with_panel: bool) -> Self {
        let mut surface = Surface::new();
        surface.time = Some(0.);
        let presentation = Rc::new(crate::presentation::Presentation::new(surface.host()));
        let panel = Rc::new(LocalArtifactsPanel::new_local(presentation.clone()));
        if let Some(filename) = filename.as_str() {
            let now = chrono::DateTime::from_timestamp_millis(0).unwrap();
            panel.artifacts().write().unwrap().insert(
                filename.into(),
                std::sync::Arc::new(std::sync::RwLock::new(
                    maestro_app::browser::tools::artifacts::Artifact {
                        filename: filename.into(),
                        content: "stored".into(),
                        created_at: now,
                        updated_at: now,
                    },
                )),
            );
        }
        presentation.opened.set(0);
        panel.show_artifact("sentinel");
        let recipe = <ArtifactsToolRenderer as ToolRenderer<Value, ()>>::render(
            &ArtifactsToolRenderer::new(with_panel.then(|| panel.clone())),
            Some(
                &json!({"command":"update", "filename":filename, "old_str":"old", "new_str":"new"}),
            ),
            None,
            false,
        )
        .unwrap();
        let mut card = Self {
            surface,
            recipe,
            state: ToolRenderState::default(),
            panel,
            presentation,
            enabled: true,
            clip: None,
        };
        card.draw();
        card.draw();
        card
    }
    fn draw(&mut self) -> egui::Response {
        let host = self.surface.host();
        self.surface.draw(|ui, frame| {
            if !self.enabled {
                ui.disable();
            }
            if let Some(clip) = self.clip {
                ui.set_clip_rect(clip);
            }
            self.recipe
                .render(ui, view(&host, frame), &mut self.state)
                .assert_no_overlays()
                .unwrap()
        })
    }
    fn button(&mut self, pos: egui::Pos2, pressed: bool) {
        self.surface.events = vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        self.draw();
    }
    fn click(&mut self, pos: egui::Pos2) {
        self.surface.events = vec![egui::Event::PointerMoved(pos)];
        self.draw();
        self.button(pos, true);
        self.button(pos, false);
    }
    fn open(&mut self, expected: bool) {
        self.draw();
        self.surface.time = Some(self.surface.time.unwrap() + 0.3);
        self.draw();
        check(if expected {
            "(()=>{const n=document.querySelector('.maestro-diff'),r=n.getBoundingClientRect();return n.contains(document.elementFromPoint(r.left+20,r.top+8))})()"
        } else {
            "(()=>{const n=document.querySelector('.maestro-diff'),r=n.getBoundingClientRect();return !n.contains(document.elementFromPoint(r.left+20,r.top+8))})()"
        });
    }
    fn header(&self) -> egui::Pos2 {
        drawing::glyphs(&self.surface)[0].center()
    }
}
fn point(selector: &str) -> egui::Pos2 {
    publish("artifactSelector", &selector.into());
    let coords = js(
        "(()=>{const r=document.querySelector(artifactSelector).getBoundingClientRect();return [r.left+r.width/2,r.top+r.height/2]})()",
    );
    let coords = js_sys::Float32Array::new(&coords);
    egui::pos2(coords.get_index(0), coords.get_index(1))
}

/// Panel actions consume both endpoints; passive pills use the existing toggle.
/// # Panics
/// Panics if selection, clipping or disclosure behavior differs.
pub fn actions() {
    recorded();
    let filename = "literal <&> 雪.rs";
    let mut card = Card::new(&filename.into(), true);
    let pill = point(".maestro-artifact-pill");
    let header = card.header();
    card.button(pill, true);
    card.button(header, false);
    card.open(false);
    assert_eq!(card.presentation.opened.get(), 0);
    card.click(pill);
    card.open(false);
    assert_eq!(card.panel.active_filename().as_deref(), Some(filename));
    assert_eq!(card.presentation.opened.get(), 1);
    card.click(header);
    card.open(true);
    card.click(pill);
    card.open(true);
    assert_eq!(card.presentation.opened.get(), 2);
    card.click(header);
    card.open(false);
    card.button(header, true);
    card.button(pill, false);
    card.open(false);
    assert_eq!(card.presentation.opened.get(), 2);
    card.button(pill, true);
    card.button(egui::pos2(790., 590.), false);
    card.open(false);
    assert_eq!(card.presentation.opened.get(), 2);
    denied_actions(&mut card, filename, pill);
    drop(card);

    let mut card = Card::new(&"passive.rs".into(), false);
    card.click(point(".maestro-artifact-pill"));
    card.open(true);
    assert_eq!(card.presentation.opened.get(), 0);
    let _ = js("document.querySelector('.maestro-artifact-pill').click()");
    card.open(false);
}

/// Filenames are literal selectable text, never formatted artifact identities.
/// # Panics
/// Panics if native selection, identity or keyboard behavior differs.
pub fn literal() {
    let filename = "<script>no()</script>  雪\t<&>.rs";
    let mut card = Card::new(&filename.into(), true);
    publish("artifactFilename", &filename.into());
    check("document.querySelector('.maestro-artifact-pill').textContent===artifactFilename");
    check("document.querySelectorAll('.maestro-artifact-pill script').length===0");
    check("document.querySelector('.maestro-artifact-pill svg')!==null");
    check(
        "(()=>{const n=document.querySelector('.maestro-artifact-pill'),r=document.createRange();r.selectNodeContents(n);getSelection().removeAllRanges();getSelection().addRange(r);return getSelection().toString().includes('雪')})()",
    );
    check("window.find('雪',false,false,true)");
    let _ = js("getSelection().removeAllRanges()");
    card.click(point(".maestro-artifact-pill"));
    assert_eq!(card.panel.active_filename().as_deref(), Some(filename));
    card.open(false);
    card.surface.size = (240., 600.);
    card.draw();
    card.draw();
    check("document.querySelector('.maestro-artifact-pill').getBoundingClientRect().height>20");
    card.open(false);
    // Focus the shared header by its ordinary pointer path, then use its keyboard route.
    card.click(card.header());
    card.open(true);
    for key in [egui::Key::Space, egui::Key::Enter] {
        card.surface.events = vec![egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }];
        card.draw();
        card.open(key == egui::Key::Enter);
        card.surface.events = vec![egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }];
        card.draw();
    }
    assert_eq!(card.presentation.opened.get(), 1);
    drop(card);
    let mut card = Card::new(&json!({"name":"display only"}), true);
    let _ = js("document.querySelector('.maestro-artifact-pill').click()");
    card.open(false);
    assert_eq!(card.panel.active_filename().as_deref(), Some("sentinel"));
    assert_eq!(card.presentation.opened.get(), 0);
}

/// Hovering an actionable filename inherits only the containing label's foreground.
/// # Panics
/// Panics if native foreground paint or the independent pill color differs.
pub async fn hover() {
    let mut card = Card::new(&"hover.rs".into(), true);
    let pack = card.surface.pack.clone();
    let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
    publish(
        "colors",
        &serde_json::to_string(&presentation.colors).unwrap().into(),
    );
    card.surface.events = vec![egui::Event::PointerMoved(point(".maestro-artifact-pill"))];
    card.draw();
    card.draw();
    publish("hoverText", &presentation.colors["text"].into());
    crate::support::reading_paint::observe("tool-header-hover").await;
    crate::tool_renderer_support::styles::native_color(
        ".maestro-artifact-pill",
        "text",
        &presentation,
    );
    card.surface.events = vec![egui::Event::PointerGone];
    card.draw();
    card.draw();
    publish("hoverText", &presentation.colors["muted"].into());
    crate::support::reading_paint::observe("tool-header-hover").await;
}

/// Both recorded direct/nested event cases retain their separate observations.
fn recorded() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../fixtures/artifact_pills.json")).unwrap();
    assert_eq!(cases.len(), 2);
    for case in cases {
        let expected = &case["expected"];
        let filename = expected["filename"].as_str().unwrap();
        let mut card = Card::new(&filename.into(), case["supplied"].as_bool().unwrap());
        card.open(!expected["nested"]["initiallyCollapsed"].as_bool().unwrap());
        let _ = js("document.querySelector('.maestro-artifact-pill').click()");
        card.open(!expected["nested"]["collapsedAfterPill"].as_bool().unwrap());
        assert_eq!(
            card.presentation.opened.get(),
            expected["nested"]["calls"].as_array().unwrap().len()
        );
        for call in expected["nested"]["calls"].as_array().unwrap() {
            assert_eq!(call.as_str(), card.panel.active_filename().as_deref());
        }
        drop(std::mem::take(&mut card.state));
        card.presentation.opened.set(0);
        card.recipe = ToolRenderResult {
            content: maestro_web::artifact_pill(
                filename.into(),
                case["supplied"]
                    .as_bool()
                    .unwrap()
                    .then(|| card.panel.clone()),
            ),
            is_custom: false,
        };
        card.draw();
        card.draw();
        publish("artifactPillExpected", &expected.to_string().into());
        check(
            "document.querySelector('.maestro-artifact-pill').textContent===JSON.parse(artifactPillExpected).filename",
        );
        check(
            "document.querySelector('.maestro-artifact-pill').classList.contains('maestro-text-action')===JSON.parse(artifactPillExpected).pointer",
        );
        let _ = js(
            "(()=>{let bubbled=0;const n=document.querySelector('.maestro-artifact-pill'),parent=n.parentElement,listener=()=>bubbled++;parent.addEventListener('click',listener);const e=new MouseEvent('click',{bubbles:true,cancelable:true});n.dispatchEvent(e);parent.removeEventListener('click',listener);globalThis.artifactClick={bubbled,prevented:e.defaultPrevented}})()",
        );
        check(
            "artifactClick.bubbled===JSON.parse(artifactPillExpected).bubbled && artifactClick.prevented===JSON.parse(artifactPillExpected).prevented",
        );
        assert_eq!(
            card.presentation.opened.get(),
            expected["calls"].as_array().unwrap().len()
        );
        for call in expected["calls"].as_array().unwrap() {
            assert_eq!(call.as_str(), card.panel.active_filename().as_deref());
        }
    }
}

/// Keep disabled, clipped and deleted filenames from admitting actions.
fn denied_actions(card: &mut Card, filename: &str, pill: egui::Pos2) {
    card.enabled = false;
    card.draw();
    card.click(pill);
    card.open(false);
    assert_eq!(card.presentation.opened.get(), 2);
    card.enabled = true;
    card.clip = Some(egui::Rect::from_min_max(
        egui::Pos2::ZERO,
        egui::pos2(30., 30.),
    ));
    card.draw();
    card.click(pill);
    card.open(false);
    assert_eq!(card.presentation.opened.get(), 2);
    card.clip = None;
    card.draw();
    crate::presentation::ready((card.panel.tool().execute)(
        "delete".into(),
        json!({"command":"delete", "filename":filename}),
        None,
        None,
    ))
    .unwrap();
    card.click(pill);
    card.open(false);
    assert_eq!(card.presentation.opened.get(), 2);
}
