//! Document observations through retained production dialogs.
#![cfg(all(test, target_arch = "wasm32"))]
use maestro_web::dialogs::dialog::{DialogHeaderProps, dialog_header};
use wasm_bindgen::prelude::*;

/// Prove header retains title description and literal text.
#[wasm_bindgen]
pub fn dialog_header_retains_title_description_and_literal_text() {
    let document = web_sys::window().unwrap().document().unwrap();
    let rows: Vec<Header> =
        serde_json::from_str(include_str!("fixtures/dialog_headers.json")).unwrap();
    for row in rows {
        let content = dialog_header(DialogHeaderProps {
            title: row.title,
            description: row.description,
            class_name: row.class_name,
        });
        let root = (content.mount)(&document).unwrap();
        let heading = root.element.query_selector("h2").unwrap().unwrap();
        assert_eq!(
            heading.text_content().as_deref(),
            Some(row.expected_title.as_str())
        );
        assert!(heading.first_element_child().is_none());
        let description = root.element.query_selector("p").unwrap();
        assert_eq!(
            description.and_then(|node| node.text_content()),
            row.expected_description
        );
        assert_eq!(heading.get_attribute("class").is_some(), row.described);
        assert_eq!(
            root.element.get_attribute("class").unwrap(),
            format!("maestro-dialog-header {}", row.expected_class)
        );
    }
}

pub mod attachment_support;
pub mod support;
use maestro_web::{
    I18n,
    browser::attachment_content::*,
    dialogs::{
        dialog::{DialogBody, DialogContentProps, DialogProps, dialog, dialog_content},
        dialog_base::{DialogBase, DialogContext},
    },
};
use std::rc::Rc;
/// Retain a production shell through document operations.
struct View {
    /// Actual replaceable native surface owner.
    host: Rc<BrowserAttachmentHost>,
    /// Renderer textures retained across updates.
    paint: std::cell::RefCell<attachment_support::dialog_paint::Paint>,
    /// Paint delivered by the most recent frame.
    shapes: std::cell::RefCell<Vec<egui::epaint::ClippedShape>>,
    /// Canvas context retained across frames.
    context: egui::Context,
    /// One weak dialog registry.
    dialogs: DialogContext,
    /// External shell lifetime.
    owner: DialogBase,
    /// Caller-owned props for explicit live updates.
    props: DialogProps,
    /// Supplied caller presentation.
    pack: maestro_theme::BrandPack,
    /// Live localized dictionary.
    i18n: I18n,
}
impl View {
    /// Bind the production dialog to the runner document.
    fn new() -> Self {
        std::panic::set_hook(Box::new(|info| {
            web_sys::console::error_1(&info.to_string().into())
        }));
        use wasm_bindgen::JsCast as _;
        let document = web_sys::window().unwrap().document().unwrap();
        let host = Rc::new(
            BrowserAttachmentHost::new(
                document
                    .get_element_by_id("canvas")
                    .unwrap()
                    .dyn_into()
                    .unwrap(),
            )
            .unwrap(),
        );
        let context = egui::Context::default();
        let dialogs = DialogContext::new(context.clone(), host.clone());
        let reading = dialog_content(DialogContentProps {
            children: vec![dialog_header(DialogHeaderProps {
                title: "<literal> title".into(),
                description: Some("Description".into()),
                class_name: String::new(),
            })],
            class_name: "content-caller".into(),
        });
        let props = DialogProps::new(
            true,
            DialogBody {
                reading,
                control_sizes: vec![],
                render_content: Rc::new(|_, _, _, _| Ok(())),
            },
        );
        let owner = dialog(&dialogs, props.clone()).unwrap();
        Self {
            paint: std::cell::RefCell::default(),
            shapes: std::cell::RefCell::default(),
            host,
            context,
            dialogs,
            owner,
            props,
            pack: maestro_theme::load_brand_pack("brand.json", &support::brand::Pack).unwrap(),
            i18n: I18n::new(Rc::new(support::Host::default())).unwrap(),
        }
    }
    /// Complete one real frame, retaining emitted paint.
    fn draw(&self) {
        self.draw_at(None);
    }
    /// Drive the existing frame with an optional controlled animation clock.
    fn draw_at(&self, time: Option<f64>) {
        self.context.begin_pass(egui::RawInput {
            time,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800., 600.),
            )),
            ..Default::default()
        });
        let result = self.dialogs.render(
            &self.i18n,
            &self
                .pack
                .presentation(maestro_theme::BrandMode::Dark)
                .unwrap(),
        );
        let mut output = self.context.end_pass();
        self.paint.borrow_mut().update(&output);
        output.textures_delta.clear();
        *self.shapes.borrow_mut() = output.shapes;
        result.unwrap();
    }
}
/// Evaluate an actual document observation.
fn js(code: &str) -> JsValue {
    js_sys::eval(code).unwrap()
}

/// Prove close name tracks live localization.
#[wasm_bindgen]
pub fn dialog_close_name_tracks_live_localization() {
    let view = View::new();
    view.draw();
    assert_eq!(
        js("document.querySelector('.maestro-dialog-semantic-close').getAttribute('aria-label')")
            .as_string()
            .as_deref(),
        Some("Close")
    );
    view.i18n.set_language("de").unwrap();
    view.draw();
    assert_eq!(
        js("document.querySelector('.maestro-dialog-semantic-close').getAttribute('aria-label')")
            .as_string()
            .as_deref(),
        Some("Schließen")
    );
}

/// Prove body keeps owned mixed content.
#[wasm_bindgen]
pub fn dialog_body_keeps_owned_mixed_content() {
    let document = web_sys::window().unwrap().document().unwrap();
    let alive = Rc::new(());
    let retained = alive.clone();
    let slot = egui::Id::new("mixed-slot");
    let child = Rc::new(ReadingContent {
        mount: Rc::new(move |document| {
            let element = document.create_element("span").unwrap();
            element.set_text_content(Some("<native> child"));
            let control = document.create_element("span").unwrap();
            element.append_child(&control).unwrap();
            Ok(ReadingRoot {
                element,
                slots: vec![(slot, control)],
                commit: None,
                owner: Some(Box::new(retained.clone())),
            })
        }),
    });
    let factory = dialog_content(DialogContentProps {
        children: vec![
            dialog_header(DialogHeaderProps {
                title: "First".into(),
                description: None,
                class_name: String::new(),
            }),
            child,
        ],
        class_name: "literal-caller".into(),
    });
    let root = (factory.mount)(&document).unwrap();
    assert_eq!(
        root.element.text_content().as_deref(),
        Some("First<native> child")
    );
    assert_eq!(root.slots.len(), 1);
    assert_eq!(root.slots[0].0, slot);
    assert!(
        root.element
            .get_attribute("class")
            .unwrap()
            .contains("literal-caller")
    );
    assert_eq!(Rc::strong_count(&alive), 3);
    drop(root);
    assert_eq!(Rc::strong_count(&alive), 2);
    let empty = dialog_content(DialogContentProps {
        children: vec![],
        class_name: String::new(),
    });
    assert_eq!(
        (empty.mount)(&document)
            .unwrap()
            .element
            .text_content()
            .as_deref(),
        Some("")
    );
}

/// Prove css size changes preserve native root and body identity.
#[wasm_bindgen]
pub async fn dialog_css_size_changes_preserve_native_root_and_body_identity() {
    let mut view = View::new();
    view.owner.close().unwrap();
    view.props.width = "min(500px, 90vw)".into();
    view.props.height = "min(700px, 90vh)".into();
    view.props.class_name = "caller-panel literal-class".into();
    view.owner = dialog(&view.dialogs, view.props.clone()).unwrap();
    js(
        "document.getElementById('canvas').style.left='13.25px';document.getElementById('canvas').style.top='7.5px'",
    );
    view.draw();
    assert_eq!(
        js("document.querySelector('.maestro-dialog-native').getAttribute('class')")
            .as_string()
            .as_deref(),
        Some("maestro-dialog-native caller-panel literal-class")
    );
    js(
        "globalThis.dialogRoot=document.querySelector('.maestro-dialog-native');globalThis.dialogHeading=dialogRoot.querySelector('h2');globalThis.editedDialogField=document.createElement('input');dialogRoot.append(editedDialogField);editedDialogField.value='retained edit';getSelection().selectAllChildren(dialogHeading)",
    );
    for (width, height) in [
        ("min(500px, 90vw)", "auto"),
        ("min(500px, 90vw)", "min(700px, 90vh)"),
        ("min(300px, 80vw)", "min(700px, 90vh)"),
    ] {
        view.props.width = width.into();
        view.props.height = height.into();
        view.owner.set_props(view.props.clone()).unwrap();
        view.draw();
        assert_eq!(js("document.querySelector('.maestro-dialog-native')===dialogRoot && getSelection().anchorNode===dialogHeading && dialogRoot.contains(editedDialogField) && editedDialogField.value==='retained edit'").as_bool(), Some(true));
    }

    view.props.width = "80vw".into();
    view.owner.set_props(view.props.clone()).unwrap();
    view.draw();
    support::reading_paint::observe("dialog-resize").await;
    view.draw();
    assert_eq!(js("document.querySelector('.maestro-dialog-native')===dialogRoot && editedDialogField.value==='retained edit' && getSelection().anchorNode===dialogHeading").as_bool(), Some(true));
    assert_eq!(js("(()=>{const r=dialogRoot.parentElement.parentElement.getBoundingClientRect();return Math.abs(r.width-innerWidth*.8)<.03 && Math.abs(r.left+r.width/2-(13.25+innerWidth/2))<.03 && Math.abs(r.top+r.height/2-(7.5+innerHeight/2))<.03})()").as_bool(), Some(true));
}

/// Prove css measurement keeps fractional axes and empty auto dimensions.
#[wasm_bindgen]
pub fn dialog_css_measurement_keeps_fractional_axes_and_empty_auto_dimensions() {
    let mut view = View::new();
    js(
        "document.getElementById('canvas').setAttribute('style','position:fixed;left:13.25px;top:7.5px;width:1000px;height:450px')",
    );
    intrinsic_sizes(&mut view);
    for (dimensions, expected) in [
        (["min(500px, 90vw)", "min(700px, 90vh)"], Some([625., 405.])),
        (
            ["calc(90vw - 0.5px)", "calc(80vh - 0.25px)"],
            Some([899.375, 359.8125]),
        ),
        (["", ""], None),
        (["min(600px, 90vw)", "auto"], None),
    ] {
        view.props.width = dimensions[0].into();
        view.props.height = dimensions[1].into();
        view.owner.set_props(view.props.clone()).unwrap();
        view.draw();
        if let Some(expected) = expected {
            let values = js_sys::Array::from(&js(
                "(()=>{const r=document.querySelector('.maestro-dialog-native').parentElement.parentElement.getBoundingClientRect();return [r.width,r.height]})()",
            ));
            assert!((values.get(0).as_f64().unwrap() - expected[0]).abs() < 0.03);
            assert!((values.get(1).as_f64().unwrap() - expected[1]).abs() < 0.03);
        }
        assert_eq!(js("(()=>{const r=document.querySelector('.maestro-dialog-native').parentElement.parentElement.getBoundingClientRect();return Math.abs((r.left+r.width/2)-(13.25+innerWidth/2*1.25))<.03&&Math.abs((r.top+r.height/2)-(7.5+innerHeight/2*.75))<.03&&r.width>0&&r.height>0})()").as_bool(), Some(true));
    }
}

/// Prove retained body replacement keeps old owner until publish.
#[wasm_bindgen]
pub fn dialog_retained_body_replacement_keeps_old_owner_until_publish() {
    let mut view = View::new();
    view.draw();
    js(
        "globalThis.oldDialogRoot=document.querySelector('.maestro-dialog-native');globalThis.oldDialogButton=oldDialogRoot.querySelector('button');getSelection().selectAllChildren(oldDialogRoot.querySelector('h2'))",
    );
    view.props.children.reading = Rc::new(ReadingContent {
        mount: Rc::new(|_| Err(AttachmentHostError::Host("candidate failure".into()))),
    });
    view.owner.set_props(view.props.clone()).unwrap();
    view.context.begin_pass(egui::RawInput::default());
    let result = view.dialogs.render(
        &view.i18n,
        &view
            .pack
            .presentation(maestro_theme::BrandMode::Dark)
            .unwrap(),
    );
    let mut output = view.context.end_pass();
    output.textures_delta.clear();
    assert_eq!(result.unwrap_err().to_string(), "candidate failure");
    assert_eq!(js("oldDialogRoot.isConnected && document.querySelector('.maestro-dialog-native')===oldDialogRoot && getSelection().toString()==='<literal> title'").as_bool(), Some(true));
    view.props.children.reading = dialog_content(DialogContentProps {
        children: vec![dialog_header(DialogHeaderProps {
            title: "Replacement".into(),
            description: None,
            class_name: String::new(),
        })],
        class_name: String::new(),
    });
    view.owner.set_props(view.props.clone()).unwrap();
    view.draw();
    assert_eq!(js("!oldDialogRoot.isConnected && document.querySelector('.maestro-dialog-native').textContent==='Replacement'").as_bool(), Some(true));
    js("oldDialogButton.click()");
    assert_eq!(
        js("document.querySelector('.maestro-dialog-native')!==null").as_bool(),
        Some(true)
    );
}

/// Prove native field escape reaches every open owner.
#[wasm_bindgen]
pub fn dialog_native_field_escape_reaches_every_open_owner() {
    use std::cell::{Cell, RefCell};
    let view = View::new();
    let mode = Rc::new(Cell::new(0));
    let behavior = mode.clone();
    let input = maestro_web::Input::new(
        maestro_web::components::input::InputProps {
            on_key_down: Some(Rc::new(move |event| match behavior.get() {
                1 => event.stop_propagation(),
                2 => event.prevent_default(),
                _ => {}
            })),
            ..Default::default()
        },
        view.host.clone(),
    );
    let input = RefCell::new(input);
    let i18n: I18n = I18n::new(Rc::new(support::Host::default())).unwrap();
    let mut props = view.props.clone();
    props.children.render_content = Rc::new(move |ui, _, _, p| {
        input.borrow_mut().render(ui, &i18n, p).map_err(|error| {
            use maestro_web::{components::input::InputUiError, dialogs::dialog::DialogUiError};
            match error {
                InputUiError::I18n(error) => DialogUiError::I18n(error),
                InputUiError::Host(error) => DialogUiError::Host(error),
                InputUiError::Presentation(error) => DialogUiError::Presentation(error),
            }
        })
    });
    let _second = dialog(&view.dialogs, props).unwrap();
    view.draw();
    js("document.querySelector('input').focus()");
    for (policy, key) in [(0, "Enter"), (0, "escape"), (1, "Escape")] {
        mode.set(policy);
        js(&format!(
            "document.querySelector('input').dispatchEvent(new KeyboardEvent('keydown',{{key:{},bubbles:true,cancelable:true}}))",
            serde_json::to_string(key).unwrap()
        ));
        view.draw();
        assert_eq!(
            js("document.querySelectorAll('.maestro-dialog-native').length").as_f64(),
            Some(2.)
        );
    }
    mode.set(2);
    js(
        "document.querySelector('input').dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',altKey:true,bubbles:true,cancelable:true}))",
    );
    view.draw();
    assert_eq!(
        js("document.querySelectorAll('.maestro-dialog-native').length").as_f64(),
        Some(0.)
    );
}

/// Prove global Escape follows native predecessor/body focus and releases its registration.
#[wasm_bindgen]
pub fn dialog_escape_from_native_predecessor_and_body_releases_listener() {
    js(
        "globalThis.dialogAdd=window.addEventListener;globalThis.dialogRemove=window.removeEventListener;globalThis.dialogKeys=new Set();window.addEventListener=function(n,f,...rest){if(n==='keydown')dialogKeys.add(f);return dialogAdd.call(this,n,f,...rest)};window.removeEventListener=function(n,f,...rest){if(n==='keydown')dialogKeys.delete(f);return dialogRemove.call(this,n,f,...rest)}",
    );
    for predecessor in [true, false] {
        js(
            "globalThis.escapePredecessor=document.createElement('button');document.body.append(escapePredecessor);escapePredecessor.focus()",
        );
        if !predecessor {
            js("escapePredecessor.blur()");
        }
        let view = View::new();
        view.draw();
        let registered = js("dialogKeys.size").as_f64();
        js(
            "document.activeElement.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true,cancelable:true}))",
        );
        view.draw();
        assert_eq!(
            js("document.querySelector('.maestro-dialog-native')===null").as_bool(),
            Some(true)
        );
        assert_eq!(registered, Some(1.));
        assert_eq!(js("dialogKeys.size").as_f64(), Some(0.));
        js("document.body.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}))");
        assert!(!view.host.take_escape());
        js("escapePredecessor.remove()");
    }
    js("window.addEventListener=dialogAdd;window.removeEventListener=dialogRemove");
}

/// Let native next-frame callbacks settle without a test deadline.
async fn next_frame() {
    use wasm_bindgen::JsCast as _;
    let promise: js_sys::Promise = js("new Promise(resolve=>requestAnimationFrame(resolve))")
        .dyn_into()
        .unwrap();
    wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
}
/// Prove focus restoration handles missing and detached targets.
#[wasm_bindgen]
pub async fn dialog_focus_restoration_handles_missing_and_detached_targets() {
    js(
        "globalThis.previousDialogFocus=document.createElement('button');previousDialogFocus.textContent='Previous';document.body.append(previousDialogFocus);previousDialogFocus.focus()",
    );
    let view = View::new();
    view.draw();
    js("document.querySelector('.maestro-dialog-semantic-close').click()");
    assert_eq!(
        js("document.querySelector('.maestro-dialog-native')===null").as_bool(),
        Some(true)
    );
    next_frame().await;
    assert_eq!(
        js("document.activeElement===previousDialogFocus").as_bool(),
        Some(true)
    );
    view.owner.close().unwrap();
    js(
        "globalThis.previousSvgFocus=document.createElementNS('http://www.w3.org/2000/svg','svg');previousSvgFocus.setAttribute('tabindex','0');document.body.append(previousSvgFocus);previousSvgFocus.focus()",
    );
    let svg = View::new();
    svg.draw();
    js("document.querySelector('.maestro-dialog-semantic-close').focus()");
    svg.owner.close().unwrap();
    next_frame().await;
    assert_eq!(
        js("document.activeElement===previousSvgFocus").as_bool(),
        Some(true)
    );
    js("previousSvgFocus.remove();previousDialogFocus.focus()");
    let detached = View::new();
    detached.draw();
    js("previousDialogFocus.remove()");
    detached.owner.close().unwrap();
    next_frame().await;
    assert_eq!(
        js("!previousDialogFocus.isConnected && document.activeElement!==previousDialogFocus")
            .as_bool(),
        Some(true)
    );
    js("Object.defineProperty(document,'activeElement',{get:()=>null, configurable:true})");
    let missing = View::new();
    js(
        "delete document.activeElement;globalThis.dialogRaf=requestAnimationFrame;globalThis.dialogFrameRegistrations=0;globalThis.requestAnimationFrame=function(callback){dialogFrameRegistrations++;return dialogRaf(callback)}",
    );
    missing.draw();
    missing.owner.close().unwrap();
    assert_eq!(js("dialogFrameRegistrations").as_f64(), Some(0.));
    js("globalThis.requestAnimationFrame=dialogRaf");
    next_frame().await;
    assert_eq!(
        js("document.querySelector('.maestro-dialog-native')===null").as_bool(),
        Some(true)
    );
}

/// Prove semantic close revokes callbacks when candidate or owner drops.
#[wasm_bindgen]
pub fn dialog_semantic_close_revokes_callbacks_when_candidate_or_owner_drops() {
    let mut view = View::new();
    view.draw();
    js(
        "globalThis.retainedClose=document.querySelector('.maestro-dialog-semantic-close');globalThis.retainedRoot=document.querySelector('.maestro-dialog-native')",
    );
    let factory = view.props.children.reading.clone();
    view.props.children.reading = dialog_header(DialogHeaderProps {
        title: "Candidate".into(),
        description: None,
        class_name: "fail-dialog-candidate".into(),
    });
    view.owner.set_props(view.props.clone()).unwrap();
    js(
        "globalThis.dialogSetAttribute=Element.prototype.setAttribute;Element.prototype.setAttribute=function(n,v){if(n==='style'&&this.matches('.maestro-reading-frame')&&this.querySelector('.fail-dialog-candidate')&&!v.includes('visibility:hidden'))throw Error('publish rejected');return dialogSetAttribute.call(this,n,v)}",
    );
    view.context.begin_pass(egui::RawInput::default());
    let result = view.dialogs.render(
        &view.i18n,
        &view
            .pack
            .presentation(maestro_theme::BrandMode::Dark)
            .unwrap(),
    );
    let mut output = view.context.end_pass();
    output.textures_delta.clear();
    js("Element.prototype.setAttribute=dialogSetAttribute");
    assert!(result.is_err());
    assert_eq!(
        js("retainedRoot.isConnected && !document.querySelector('.fail-dialog-candidate')")
            .as_bool(),
        Some(true)
    );
    js("retainedClose.click()");
    assert_eq!(js("!retainedRoot.isConnected").as_bool(), Some(true));
    view.owner.open().unwrap();
    view.draw();
    js("globalThis.droppedClose=document.querySelector('.maestro-dialog-semantic-close')");
    drop(view.owner);
    js("droppedClose.click();retainedClose.click()");
    assert_eq!(
        js("document.querySelector('.maestro-dialog-native')===null").as_bool(),
        Some(true)
    );
    drop(factory);
}

/// Prove first frame uses current body size and slot geometry.
#[wasm_bindgen]
pub fn dialog_first_frame_uses_current_body_size_and_slot_geometry() {
    use std::cell::RefCell;
    let mut view = View::new();
    let observed = Rc::new(RefCell::new(Vec::new()));
    let record = observed.clone();
    let slot = egui::Id::new("first-frame-control");
    view.props.children.reading = Rc::new(ReadingContent {
        mount: Rc::new(move |document| {
            let element = document.create_element("div").unwrap();
            element.set_text_content(Some("Natural native body with a changing control slot."));
            let child = document.create_element("span").unwrap();
            child.set_attribute("style", "display:block").unwrap();
            child.set_id("dialog-body-slot");
            element.append_child(&child).unwrap();
            Ok(ReadingRoot {
                element,
                slots: vec![(slot, child)],
                commit: None,
                owner: None,
            })
        }),
    });
    view.props.children.render_content = Rc::new(move |ui, _, layout, _| {
        let full = layout.controls[0].1.translate(ui.max_rect().min.to_vec2());
        record.borrow_mut().push(full);
        Ok(())
    });
    for (width, size) in [
        ("min(400px, 90vw)", egui::vec2(30., 24.)),
        ("min(220px, 90vw)", egui::vec2(60., 70.)),
    ] {
        view.props.width = width.into();
        view.props.children.control_sizes = vec![(slot, size)];
        view.owner.set_props(view.props.clone()).unwrap();
        view.draw();
        let native = js(
            "(()=>{const r=document.getElementById('dialog-body-slot').getBoundingClientRect();return [r.x,r.y,r.width,r.height]})()",
        );
        let native = js_sys::Array::from(&native);
        let last = *observed.borrow().last().unwrap();
        for (actual, expected) in [last.left(), last.top(), last.width(), last.height()]
            .into_iter()
            .zip((0..4).map(|i| native.get(i).as_f64().unwrap()))
        {
            assert!((f64::from(actual) - expected).abs() < 0.02);
        }
        let shapes = view.shapes.borrow();
        assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(r) if r.stroke.width==1. && r.rect.width()>200.)));
    }
}

/// Prove reading text is findable selectable and accessible.
#[wasm_bindgen]
pub async fn dialog_reading_text_is_findable_selectable_and_accessible() {
    let view = View::new();
    view.draw();
    js("getSelection().selectAllChildren(document.querySelector('.maestro-dialog-native h2'))");
    assert_eq!(
        js("getSelection().toString()").as_string().as_deref(),
        Some("<literal> title")
    );
    support::reading_paint::observe("dialog-accessible").await;
}

/// Prove semantic close routes keyboard and accessibility to canvas owner.
#[wasm_bindgen]
pub async fn dialog_semantic_close_routes_keyboard_and_accessibility_to_canvas_owner() {
    for scene in ["dialog-keyboard-enter", "dialog-keyboard-space"] {
        let view = View::new();
        view.draw();
        support::reading_paint::observe(scene).await;
        view.owner.close().unwrap();
    }
    let view = View::new();
    view.draw();
    js("document.querySelector('.maestro-dialog-semantic-close').click()");
    assert_eq!(
        js("document.querySelector('.maestro-dialog-native')===null").as_bool(),
        Some(true)
    );
}

/// Prove that the renderer receives a nondegenerate visible triangle, not a hidden declaration.
fn visible(view: &View, shape: &egui::epaint::ClippedShape, outside: Option<egui::Rect>) -> bool {
    view.context
        .tessellate(vec![shape.clone()], 1.)
        .iter()
        .any(|primitive| {
            let egui::epaint::Primitive::Mesh(mesh) = &primitive.primitive else {
                return false;
            };
            mesh.indices.chunks_exact(3).any(|indices| {
                let [a, b, c] = [
                    mesh.vertices[indices[0] as usize],
                    mesh.vertices[indices[1] as usize],
                    mesh.vertices[indices[2] as usize],
                ];
                let center = a.pos + ((b.pos - a.pos) + (c.pos - a.pos)) / 3.;
                (b.pos - a.pos).rot90().dot(c.pos - a.pos).abs() > 0.
                    && primitive.clip_rect.shrink(0.01).contains(center)
                    && [a, b, c].iter().any(|vertex| vertex.color.a() > 0)
                    && outside.is_none_or(|rect| !rect.contains(center))
            })
        })
}
/// Prove semantic close paints nothing and tracks canvas focus.
#[wasm_bindgen]
pub async fn dialog_semantic_close_paints_nothing_and_tracks_canvas_focus() {
    let view = View::new();
    view.draw();
    support::reading_paint::observe("dialog-semantic").await;
    js("document.querySelector('.maestro-dialog-semantic-close').focus()");
    for _ in 0..12 {
        view.draw();
    }
    let presentation = view
        .pack
        .presentation(maestro_theme::BrandMode::Dark)
        .unwrap();
    for (distance, role) in [(0.5, "background"), (2.5, "accent")] {
        let color = view.paint.borrow().pixel(
            &view.context,
            &view.shapes.borrow(),
            egui::pos2(
                native_close(&view).center().x,
                native_close(&view).top() - distance,
            ),
        );
        let expected = egui::Color32::from_hex(presentation.colors[role])
            .unwrap()
            .gamma_multiply(0.7);
        for (actual, expected) in color.into_iter().zip(expected.to_array()) {
            assert!((actual - f32::from(expected)).abs() <= 1.);
        }
    }
    js("document.querySelector('.maestro-dialog-semantic-close').blur()");
    for _ in 0..12 {
        view.draw();
    }
    assert!(native_rings(&view).is_empty());
}

/// Prove decoration survives clipping and scroll.
#[wasm_bindgen]
pub fn dialog_decoration_survives_clipping_and_scroll() {
    let mut view = View::new();
    view.props.height = "180px".into();
    view.props.children.reading = dialog_header(DialogHeaderProps {
        title: "Heading".into(),
        description: Some("A long scrolling body. ".repeat(200)),
        class_name: String::new(),
    });
    view.owner.set_props(view.props.clone()).unwrap();
    view.draw();
    let before: Vec<_> = view.shapes.borrow().iter().filter(|shape| matches!(&shape.shape, egui::Shape::Rect(r) if r.blur_width>0. || r.stroke.width==1.)).cloned().collect();
    assert_eq!(before.len(), 3);
    let panel = egui::Rect::from_min_size(egui::pos2(100., 210.), egui::vec2(600., 180.));
    let presentation = view
        .pack
        .presentation(maestro_theme::BrandMode::Dark)
        .unwrap();
    let border = egui::Color32::from_hex(presentation.colors["border"]).unwrap();
    for ((shape, expected), blur) in before[..2]
        .iter()
        .zip([
            panel.translate(egui::vec2(0., 1.)),
            panel.shrink(1.).translate(egui::vec2(0., 8.)),
        ])
        .zip([3., 10.])
    {
        assert!(visible(&view, shape, Some(panel)));
        let egui::Shape::Rect(shadow) = &shape.shape else {
            unreachable!()
        };
        assert_eq!(shadow.rect, expected);
        assert_eq!(shadow.blur_width, blur);
        assert_eq!(shadow.fill, border.gamma_multiply(0.1));
    }
    assert!(visible(&view, &before[2], None));
    let egui::Shape::Rect(frame) = &before[2].shape else {
        unreachable!()
    };
    assert_eq!(frame.rect, panel);
    assert_eq!(frame.stroke.color, border);
    js(
        "globalThis.closeBeforeScroll=document.querySelector('.maestro-dialog-semantic-close').getBoundingClientRect().toJSON()",
    );
    js(
        "globalThis.dialogFrame=document.querySelector('.maestro-dialog-native').parentElement.parentElement;globalThis.dialogFrameStyle=dialogFrame.getAttribute('style');dialogFrame.firstElementChild.scrollTop=50",
    );
    view.draw();
    let after: Vec<_> = view.shapes.borrow().iter().filter(|shape| matches!(&shape.shape, egui::Shape::Rect(r) if r.blur_width>0. || r.stroke.width==1.)).cloned().collect();
    assert_eq!(before, after);
    assert_eq!(js("dialogFrame.firstElementChild.scrollTop>0 && dialogFrame.getAttribute('style')===dialogFrameStyle").as_bool(), Some(true));
    assert_eq!(js("(()=>{const r=document.querySelector('.maestro-dialog-semantic-close').getBoundingClientRect();return Math.abs(r.x-closeBeforeScroll.x)<.02&&Math.abs(r.y-closeBeforeScroll.y)<.02})()").as_bool(), Some(true));
}

/// Source-derived literal header cases with no unconsumed fixture fields.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    /// Authored heading.
    title: String,
    /// Authored optional paragraph.
    description: Option<String>,
    /// Authored wrapper classes.
    class_name: String,
    /// Observed literal heading text.
    expected_title: String,
    /// Observed optional paragraph text.
    expected_description: Option<String>,
    /// Observed wrapper classes.
    expected_class: String,
    /// Whether the heading receives paragraph spacing.
    described: bool,
}

/// Compare literal native intrinsic layout against the same body under one frame scale.
fn intrinsic_sizes(view: &mut View) {
    view.props.children.reading = dialog_content(DialogContentProps {
        children: vec![dialog_header(DialogHeaderProps {
            title: "alpha beta gamma delta ".repeat(20),
            description: None,
            class_name: String::new(),
        })],
        class_name: String::new(),
    });
    for keyword in ["min-content", "max-content", "fit-content", "auto", ""] {
        view.props.width = keyword.into();
        view.owner.set_props(view.props.clone()).unwrap();
        view.draw();
        let observation = js(&format!(
            r"(()=>{{const actual=document.querySelector('.maestro-dialog-native').parentElement.parentElement;const oracle=actual.cloneNode(true);oracle.style.cssText+=';position:fixed;left:50vw;top:50vh;width:auto;width:{keyword};height:auto;transform:translate(-50%,-50%);clip-path:none;visibility:hidden';document.body.append(oracle);const expected=oracle.getBoundingClientRect(),result=actual.getBoundingClientRect(),button=actual.querySelector('button').getBoundingClientRect();const out=[result.width,expected.width*1.25,result.height,expected.height*.75,button.width,button.height,result.left,result.top,button.left,button.top];oracle.remove();return out}})()"
        ));
        let values = js_sys::Array::from(&observation);
        intrinsic_geometry(view, &values);
    }
}

/// Match native bounds and Close extents to the emitted canvas geometry.
fn intrinsic_geometry(view: &View, values: &js_sys::Array) {
    let panel = view
        .shapes
        .borrow()
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect) if (rect.stroke.width - 1.).abs() < f32::EPSILON => {
                Some(rect.rect)
            }
            _ => None,
        })
        .unwrap();
    for (index, expected) in [
        (6, 13.25 + f64::from(panel.left()) * 1.25),
        (7, 7.5 + f64::from(panel.top()) * 0.75),
        (8, 13.25 + f64::from(panel.right() - 33.) * 1.25),
        (9, 7.5 + f64::from(panel.top() + 17.) * 0.75),
    ] {
        assert!((values.get(index).as_f64().unwrap() - expected).abs() < 0.03);
    }

    for (actual, expected) in [
        (
            values.get(0).as_f64().unwrap(),
            values.get(1).as_f64().unwrap(),
        ),
        (
            values.get(2).as_f64().unwrap(),
            values.get(3).as_f64().unwrap(),
        ),
        (values.get(4).as_f64().unwrap(), 20.),
        (values.get(5).as_f64().unwrap(), 12.),
    ] {
        assert!(
            (actual - expected).abs() < 0.03,
            "native intrinsic sizing must be evaluated before the single frame scale"
        );
    }
}

/// Native keyboard focus follows continuous shortened shadow paint and resets on reopen.
#[wasm_bindgen]
pub async fn dialog_native_focus_transition_retargets_and_resets() {
    for (time, width, next_width) in [
        (0.0375, 0.473_174, 0.125_703_6),
        (0.075, 1.551_123, 1.459_396),
        (0.1125, 1.918_735, 1.849_56),
    ] {
        let view = View::new();
        view.draw_at(Some(0.));
        support::reading_paint::observe("dialog-keyboard-focus").await;
        view.draw_at(Some(1.));
        view.draw_at(Some(1. + time));
        let before = native_rings(&view);
        assert_native_spread(&view, width);
        js("document.querySelector('.maestro-dialog-semantic-close').blur()");
        view.draw_at(Some(1. + time));
        assert_eq!(native_rings(&view), before);
        view.draw_at(Some(1. + time + 1. / 60.));
        assert_native_spread(&view, next_width);
        view.owner.close().unwrap();
        view.owner.open().unwrap();
        view.draw_at(Some(2.));
        assert!(native_rings(&view).is_empty());
        view.draw_at(Some(2. + 1. / 60.));
        assert!(native_rings(&view).is_empty());
    }
}

/// Collect only the Close shadow annuli from the actual submitted paint.
fn native_rings(view: &View) -> Vec<f32> {
    view.paint
        .borrow()
        .top_rows(&view.context, &view.shapes.borrow(), native_close(view))
}

/// Read the actual canvas glyph extent, independent of the native body's natural height.
fn native_close(view: &View) -> egui::Rect {
    attachment_support::dialog_paint::glyphs(&view.shapes.borrow())
        .next()
        .unwrap()
        .calc_bounds()
}

/// Compare actual keyboard-focus pixel coverage with the recorded fractional spread.
fn assert_native_spread(view: &View, spread: f32) {
    attachment_support::dialog_paint::assert_spread(
        view.paint
            .borrow()
            .top_alpha(&view.context, &view.shapes.borrow(), native_close(view)),
        spread,
        0.7,
    );
}
