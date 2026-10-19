//! Actual-document transcript containment through the retained reading registry.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::reading_slots_support::{Slots, js, snapshot, unchanged};
use maestro_web::browser::attachment_content::{
    AttachmentContent, AttachmentHost, AttachmentHostError, AttachmentSurface, ReadingContent,
    ReadingPhase, ReadingPlacement, ReadingRoot,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::prelude::*;
use web_sys::Element;

/// Assert an actual-document observation, preserving native node handles.
fn check(value: &JsValue, code: &str) {
    assert!(js(value, code).as_bool().unwrap(), "{code}");
}
/// Select real text and retain nonzero native offsets on both axes.
fn select_and_scroll(root: &Element) {
    js(
        root,
        "const viewport=value.parentElement;viewport.scrollLeft=9.5;viewport.scrollTop=31.5;const r=document.createRange();r.selectNodeContents(value.querySelector('h1'));getSelection().removeAllRanges();getSelection().addRange(r)",
    );
    check(
        root,
        "return value.parentElement.scrollLeft>0 && value.parentElement.scrollTop>0 && getSelection().toString().length>0",
    );
}
/// An owned row with a nested native child, mounted in the supplied actual document.
fn mount_slot(document: &web_sys::Document, index: usize) -> (egui::Id, Element) {
    let element = document.create_element("div").unwrap();
    element.set_text_content(Some(&format!("appended {index}")));
    let nested = document.create_element("span").unwrap();
    nested.set_text_content(Some("nested owner"));
    element.append_child(&nested).unwrap();
    (egui::Id::new(("added", index)), element)
}
/// Positional growth is atomic and preserves untouched native roots and browser state.
/// # Panics
/// Panics if mounting, native identity, release or rollback differs.
#[wasm_bindgen]
pub fn transcript_registry_appends_and_retires_slots_without_replacing_native_roots() {
    let slots = Slots::new();
    slots.publish(egui::vec2(300., 80.), &Slots::sizes([(30., 20.); 3]));
    select_and_scroll(&slots.root());
    assert_scroll_observations(&slots);
    let saved = snapshot(&slots.root());
    let calls = Cell::new(0);
    let unrelated = egui::Id::new("unrelated transcript owner");
    let mount = |document: &web_sys::Document, index| {
        calls.set(calls.get() + 1);
        slots.host.set_retained_visibility(unrelated, true, None)?;
        Ok(mount_slot(document, index))
    };
    slots
        .host
        .resize_reading_slots(slots.id, 5, &mount)
        .unwrap();
    unchanged(&saved);
    assert_eq!(calls.get(), 2);
    let roots = js(&slots.root(), "return [...value.children].slice(-2)");
    check(
        &roots,
        "return value.map(n=>n.firstChild.textContent).join('|')==='appended 3|appended 4'",
    );
    slots
        .host
        .resize_reading_slots(slots.id, 5, &mount)
        .unwrap();
    assert_eq!(calls.get(), 2);
    slots
        .host
        .resize_reading_slots(slots.id, 4, &mount)
        .unwrap();
    check(
        &roots,
        "return value[0].isConnected && !value[1].isConnected && !value[1].lastChild.isConnected",
    );
    let before_failure = snapshot(&slots.root());
    let failed = slots
        .host
        .resize_reading_slots(slots.id, 6, &|document, index| {
            if index == 5 {
                return Err(AttachmentHostError::Host("factory failed".into()));
            }
            mount(document, index)
        });
    assert!(matches!(failed, Err(AttachmentHostError::Host(_))));
    unchanged(&before_failure);
    check(
        &slots.root(),
        "return value.lastChild.firstChild.textContent==='appended 3'",
    );
    assert_append_rollback(&slots);
    slots.host.remove(unrelated);
    assert_independent_release(slots, &roots);
}

/// Fractional read/write uses the actual viewport and survives repeated publication.
fn assert_scroll_observations(slots: &Slots) {
    let viewport = slots.wrapper();
    slots.host.set_reading_scroll(slots.id, 42.5).unwrap();
    let observed = slots.host.reading_scroll(slots.id).unwrap();
    let native = js(
        &viewport,
        "return [value.scrollTop,value.scrollHeight,value.clientHeight,value.scrollLeft]",
    );
    let values = js_sys::Float64Array::new(&native);
    assert_eq!(observed.top.to_bits(), values.get_index(0).to_bits());
    assert_eq!(observed.height.to_bits(), values.get_index(1).to_bits());
    assert_eq!(
        observed.client_height.to_bits(),
        values.get_index(2).to_bits()
    );
    let saved = snapshot(&slots.root());
    slots.publish(egui::vec2(300., 80.), &Slots::sizes([(30., 20.); 3]));
    unchanged(&saved);
    assert_eq!(slots.host.reading_scroll(slots.id).unwrap(), observed);
    assert!(slots.host.reading_scroll(egui::Id::new("absent")).is_err());
    assert!(
        slots
            .host
            .set_reading_scroll(egui::Id::new("absent"), 0.)
            .is_err()
    );
    js(
        &viewport,
        "Object.defineProperty(value,'scrollTop',{configurable:true,set(){throw Error('scroll failure')}})",
    );
    assert!(matches!(
        slots.host.set_reading_scroll(slots.id, 1.),
        Err(AttachmentHostError::Host(_))
    ));
    js(&viewport, "delete value.scrollTop");
    unchanged(&saved);
}

/// Releasing one containing tree detaches its descendants without releasing another instance.
fn assert_independent_release(slots: Slots, roots: &JsValue) {
    let second = Slots::new();
    second.publish(egui::vec2(300., 80.), &[]);
    drop(slots);
    check(
        roots,
        "return value.every(n=>!n.isConnected && !n.lastChild.isConnected)",
    );
    assert!(second.root().is_connected());
    let second_root = second.root();
    drop(second);
    assert!(!second_root.is_connected());
}

/// A failed native append rolls back even additions already attached in that operation.
fn assert_append_rollback(slots: &Slots) {
    let saved = snapshot(&slots.root());
    let before = js(&slots.root(), "return [...value.childNodes]");
    js(
        &slots.root(),
        "const original=value.appendChild;let calls=0;value.appendChild=function(node){if(++calls===2){value.appendChild=original;throw Error('append failed')}return original.call(this,node)}",
    );
    let failed = slots
        .host
        .resize_reading_slots(slots.id, 6, &|document, index| {
            Ok(mount_slot(document, index))
        });
    assert!(matches!(failed, Err(AttachmentHostError::Host(_))));
    unchanged(&saved);
    let after = js(&slots.root(), "return [...value.childNodes]");
    let pair = js_sys::Array::of2(&before, &after);
    check(
        &pair,
        "return value[0].length===value[1].length && value[0].every((node,i)=>node===value[1][i])",
    );
}

/// A retained reading owner placed inside an existing registry slot.
struct Nested {
    /// Standalone scrolling owner and its actual-document harness.
    outer: Slots,
    /// Native factory identity shared through repeated publications.
    content: Rc<ReadingContent>,
    /// Mirror of the mounted inner root, without replacing its owner.
    mounted: Rc<RefCell<Option<ReadingRoot>>>,
    /// Actual containing slot identity.
    slot: egui::Id,
    /// Inner surface identity.
    id: egui::Id,
    /// Canvas placement updated by native scrolling.
    rect: Cell<egui::Rect>,
}
impl Nested {
    /// Admit independent canvas axis scales and fractional origin coordinates.
    fn new() -> Self {
        let mut outer = Slots::new();
        outer.origin = egui::pos2(12.5, 18.25);
        outer.clip = egui::Rect::from_min_size(outer.origin, egui::vec2(220., 65.));
        outer.density = 1.25;
        js(
            &JsValue::NULL,
            "const canvas=document.getElementById('canvas');canvas.style.width='1000px';canvas.style.height='450px'",
        );
        let layout = outer.publish(egui::vec2(300., 100.), &Slots::sizes([(260., 350.); 3]));
        js(
            &outer.root(),
            "value.querySelector('style').textContent += '.test-slot{position:relative}'",
        );
        let mounted = Rc::new(RefCell::new(None));
        let content = Slots::factory(Rc::clone(&mounted));
        let slot = outer.root.borrow().as_ref().unwrap().slots[0].0;
        let origin = outer.origin + layout.controls[0].1.min.to_vec2();
        Self {
            outer,
            content,
            mounted,
            slot,
            id: egui::Id::new("nested transcript row"),
            rect: Cell::new(egui::Rect::from_min_size(origin, egui::vec2(260., 300.))),
        }
    }
    /// Submit a phase through the actual containing scope, not DOM postprocessing.
    fn place(&self, phase: ReadingPhase) -> Result<(), AttachmentHostError> {
        let _container =
            self.outer
                .host
                .enter_reading_container(self.outer.id, Some(self.slot), None)?;
        self.outer.host.present(
            AttachmentSurface {
                id: self.id,
                rect: self.rect.get(),
                clip_rect: self.outer.clip,
                pixels_per_point: self.outer.density,
                opacity: 1.,
                occluders: vec![],
                content: AttachmentContent::Reading(ReadingPlacement {
                    content: Rc::clone(&self.content),
                    phase,
                    intrinsic_width: false,
                    sizes: &Slots::sizes([(20., 15.); 3]),
                    modal: None,
                    context: &self.outer.context,
                }),
            },
            &self
                .outer
                .pack
                .presentation(maestro_theme::BrandMode::Dark)
                .unwrap(),
        )
    }
    /// Complete the same measure/readback/publication sequence used by row consumers.
    fn publish(&self) -> Result<(), AttachmentHostError> {
        self.place(ReadingPhase::Measure)?;
        assert_eq!(self.outer.host.reading_layout(self.id)?.controls.len(), 3);
        self.place(ReadingPhase::Publish)
    }
    /// Read the actual retained inner root.
    fn root(&self) -> Element {
        self.mounted.borrow().as_ref().unwrap().element.clone()
    }
    /// Account for the observed scroll in the next canvas placement, preserving local geometry.
    fn scrolled(&self) {
        select_and_scroll(&self.outer.root());
        let offset = js(
            &self.outer.root(),
            "return JSON.stringify([value.parentElement.scrollLeft,value.parentElement.scrollTop])",
        );
        let offset: [f32; 2] = serde_json::from_str(&offset.as_string().unwrap()).unwrap();
        self.rect
            .set(self.rect.get().translate(-egui::vec2(offset[0], offset[1])));
    }
}
impl Drop for Nested {
    fn drop(&mut self) {
        self.outer.host.remove(self.id);
    }
}
/// Both failure phases restore existing placement, selection and native offsets.
fn assert_nested_rollback(nested: &Nested) {
    let root = nested.root();
    let saved = snapshot(&nested.outer.root());
    let inner_saved = snapshot(&root);
    js(
        &root,
        "value.savedBounds=value.getBoundingClientRect;value.getBoundingClientRect=()=>{throw Error('measure failure')}",
    );
    assert!(nested.publish().is_err());
    js(&root, "value.getBoundingClientRect=value.savedBounds");
    unchanged(&saved);
    unchanged(&inner_saved);
    nested.place(ReadingPhase::Measure).unwrap();
    nested.outer.host.reading_layout(nested.id).unwrap();
    js(
        &root,
        "const frame=value.parentElement.parentElement;const saved=frame.setAttribute;frame.setAttribute=function(k,v){frame.setAttribute=saved;throw Error('publish failure')}",
    );
    assert!(nested.place(ReadingPhase::Publish).is_err());
    unchanged(&saved);
    unchanged(&inner_saved);
    nested.publish().unwrap();
}
/// Nested native frames share containment, independent canvas axes and rollback boundaries.
/// # Panics
/// Panics if placement, selection, scrolling, modal policy or failure restoration differs.
#[wasm_bindgen]
pub fn transcript_registry_containment_keeps_nested_slots_and_rolls_back_failures() {
    let nested = Nested::new();
    nested.publish().unwrap();
    let root = nested.root();
    check(
        &root,
        "return value.closest('.test-slot')!==null && value.parentElement.parentElement.style.position==='absolute'",
    );
    nested.scrolled();
    let saved = snapshot(&nested.outer.root());
    nested.publish().unwrap();
    unchanged(&saved);
    assert!(nested.root().is_same_node(Some(&root)));
    check(
        &root,
        "const frame=value.parentElement.parentElement;const slot=frame.parentElement;const a=frame.getBoundingClientRect(),b=slot.getBoundingClientRect();return Math.abs(a.left-b.left)<0.1 && Math.abs(a.top-b.top)<0.1 && Math.abs(a.width-406.25)<0.1 && Math.abs(a.height-281.25)<0.1",
    );
    check(
        &root,
        "const r=value.querySelector('h1').getBoundingClientRect();return !value.contains(document.elementFromPoint(r.right-1,r.top+1))",
    );
    let modal = egui::Id::new("transcript modal");
    nested.outer.host.set_overlay(modal, true).unwrap();
    check(
        &root,
        "return getComputedStyle(value).visibility==='hidden'",
    );
    nested.outer.host.set_overlay(modal, false).unwrap();
    check(
        &root,
        "return getComputedStyle(value).visibility==='visible'",
    );
    assert_nested_rollback(&nested);
    // Standalone consumers retain their own fixed, independently scrolling viewport.
    check(
        &nested.outer.root(),
        "return value.parentElement.parentElement.style.position==='fixed' && getComputedStyle(value.parentElement).overflow==='auto'",
    );
    drop(nested);
    assert!(!root.is_connected());
}

/// Changed preceding heights do not offset a child twice when sizes publish afterward.
/// # Panics
/// Panics when a planned row origin is replaced with the previous native slot bounds.
#[wasm_bindgen]
pub fn transcript_registry_positions_rows_before_publishing_their_new_heights() {
    let nested = Nested::new();
    let layout = nested.outer.host.reading_layout(nested.outer.id).unwrap();
    let slot = nested.outer.root.borrow().as_ref().unwrap().slots[1].0;
    let origin = nested.outer.origin + layout.controls[1].1.min.to_vec2() + egui::vec2(0., 150.);
    let rect = egui::Rect::from_min_size(origin, egui::vec2(260., 300.));
    {
        let _scope = nested
            .outer
            .host
            .enter_reading_container(nested.outer.id, Some(slot), Some(origin))
            .unwrap();
        for phase in [ReadingPhase::Measure, ReadingPhase::Publish] {
            nested
                .outer
                .host
                .present(
                    AttachmentSurface {
                        id: nested.id,
                        rect,
                        clip_rect: nested.outer.clip,
                        pixels_per_point: nested.outer.density,
                        opacity: 1.,
                        occluders: vec![],
                        content: AttachmentContent::Reading(ReadingPlacement {
                            content: Rc::clone(&nested.content),
                            phase,
                            intrinsic_width: false,
                            sizes: &[],
                            modal: None,
                            context: &nested.outer.context,
                        }),
                    },
                    &nested.outer.pack.presentation(nested.outer.mode).unwrap(),
                )
                .unwrap();
        }
    }
    nested.outer.publish(
        egui::vec2(300., 100.),
        &Slots::sizes([(260., 500.), (260., 350.), (260., 350.)]),
    );
    check(
        &nested.root(),
        "const frame=value.parentElement.parentElement;const a=frame.getBoundingClientRect(),b=frame.parentElement.getBoundingClientRect();return Math.abs(a.left-b.left)<0.1 && Math.abs(a.top-b.top)<0.1",
    );
}

#[path = "transcript_browser/aggregate.rs"]
pub(crate) mod aggregate;

/// Later publication must not undo a reader scroll after an earlier programmatic assignment.
/// # Panics
/// Panics when a retired programmatic offset wins over the current native viewport.
#[wasm_bindgen]
pub fn transcript_registry_publication_keeps_reader_scroll_after_programmatic_follow() {
    let slots = Slots::new();
    let size = egui::vec2(300., 80.);
    let sizes = Slots::sizes([(30., 200.); 3]);
    slots.publish(size, &sizes);
    slots.host.set_reading_scroll(slots.id, 80.).unwrap();
    js(&slots.wrapper(), "value.scrollTop=30.5");
    let expected = slots.host.reading_scroll(slots.id).unwrap();
    for phase in [ReadingPhase::Measure, ReadingPhase::Publish] {
        slots.place(phase, size, &sizes).unwrap();
    }
    assert_eq!(slots.host.reading_scroll(slots.id).unwrap(), expected);
}
