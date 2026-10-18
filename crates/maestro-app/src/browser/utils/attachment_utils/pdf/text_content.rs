//! Logical text shows decoded through the native font interpreter.
use hayro::hayro_syntax::{
    content::{TypedIter, ops::TypedInstruction},
    object::{Array, Dict, Name, Object, ObjectIdentifier},
    page::{Page, Resources},
};
use hayro::kurbo::{Affine, BezPath, Rect};
use hayro_interpret::{
    BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image, ImageDrawProps,
    InterpreterCache, InterpreterSettings, SoftMask, font::GlyphRun, hayro_cmap::BfString,
    interpret,
};
use std::{
    collections::{HashMap, HashSet},
    fmt::Write,
};

/// Resource namespace that selects a font.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
enum FontSource {
    /// A text-font resource.
    Font,
    /// An extended graphics state carrying a font dictionary.
    GraphicsState,
}
/// One retained resource binding and its decoding stream.
struct Batch<'a> {
    /// Resources at selection, including Form-local bindings.
    resources: Resources<'a>,
    /// Byte-preserving font selection command.
    setup: String,
    /// Original shows and decoder state transitions.
    content: String,
    /// Page-order output slots, including empty shows.
    slots: Vec<usize>,
}
/// Byte operands and native fallback semantics of a logical show.
#[derive(Clone, Copy)]
enum TextShow<'b, 'a> {
    /// A single string with ordinary-show fallback.
    Text(&'b [u8]),
    /// Individually decoded strings with ordinary-show fallback.
    Array(&'b Array<'a>),
    /// A quote operator without ordinary-show fallback.
    Quote(&'b [u8]),
}
/// Page-order shows indexed by the resources that selected their fonts.
#[derive(Default)]
struct Shows<'a> {
    /// Native decoding groups.
    batches: Vec<Batch<'a>>,
    /// Resource dictionary identity, raw name and resource namespace.
    by_font: HashMap<(usize, Vec<u8>, FontSource), usize>,
    /// Form streams executing on the current traversal path.
    active_forms: HashSet<ObjectIdentifier>,
    /// Number of original logical shows.
    count: usize,
}
impl<'a> Shows<'a> {
    /// Select the original resource binding and reset its decoder font state.
    fn select(&mut self, resources: &Resources<'a>, name: &[u8], source: FontSource) -> usize {
        let dict = match source {
            FontSource::Font => &resources.fonts,
            FontSource::GraphicsState => &resources.ext_g_states,
        };
        let key = (dict.data().as_ptr() as usize, name.to_vec(), source);
        if let Some(&index) = self.by_font.get(&key) {
            let batch = &mut self.batches[index];
            batch.content.push_str(&batch.setup);
            return index;
        }
        let mut setup = String::from("/");
        for byte in name {
            let _ = write!(setup, "#{byte:02X}");
        }
        setup.push_str(match source {
            FontSource::Font => " 12 Tf\n",
            FontSource::GraphicsState => " gs\n",
        });
        let index = self.batches.len();
        self.batches.push(Batch {
            resources: resources.clone(),
            content: setup.clone(),
            setup,
            slots: Vec::new(),
        });
        self.by_font.insert(key, index);
        index
    }

    /// Preserve one show boundary and each string's original encoded bytes.
    fn add(&mut self, font: usize, show: TextShow<'_, '_>) {
        let batch = &mut self.batches[font];
        batch.slots.push(self.count);
        batch.content.push_str("/Span BMC ");
        match show {
            TextShow::Quote(bytes) => {
                write_hex(&mut batch.content, bytes);
                batch.content.push_str("' EMC\n");
            }
            TextShow::Text(bytes) => {
                batch.content.push('[');
                write_hex(&mut batch.content, bytes);
                batch.content.push_str("] TJ EMC\n");
            }
            TextShow::Array(array) => {
                batch.content.push('[');
                for string in array.iter::<Object<'_>>().filter_map(Object::into_string) {
                    write_hex(&mut batch.content, string.as_bytes());
                }
                batch.content.push_str("] TJ EMC\n");
            }
        }
        self.count += 1;
    }

    /// Traverse typed content while retaining selected and saved font bindings.
    fn walk(&mut self, mut ops: TypedIter<'_>, resources: &Resources<'a>, mut font: Option<usize>) {
        let mut stack = Vec::new();
        while let Some(op) = ops.next() {
            match op {
                TypedInstruction::SaveState(_) => {
                    stack.push(font);
                    self.state(font, "q\n");
                }
                TypedInstruction::RestoreState(_) if !stack.is_empty() => {
                    font = stack.pop().flatten();
                    self.state(font, "Q\n");
                }
                TypedInstruction::TextFont(value) => {
                    font = Some(self.select(resources, value.0, FontSource::Font));
                }
                TypedInstruction::SetGraphicsState(value)
                    if graphics_state_has_font(resources, value.0) =>
                {
                    font = Some(self.select(resources, value.0, FontSource::GraphicsState));
                }
                TypedInstruction::XObject(value) => self.form(resources, value.0, font),
                other => self.show(font, other),
            }
        }
    }

    /// Dispatch the four text-show operators without flattening string operands.
    fn show(&mut self, font: Option<usize>, op: TypedInstruction<'_, '_>) {
        let Some(font) = font else {
            return;
        };
        match op {
            TypedInstruction::ShowText(value) => self.add(font, TextShow::Text(value.0.as_bytes())),
            TypedInstruction::ShowTexts(value) => self.add(font, TextShow::Array(value.0)),
            TypedInstruction::NextLineAndShowText(value) => {
                self.add(font, TextShow::Quote(value.0.as_bytes()));
            }
            TypedInstruction::ShowTextWithParameters(value) => {
                self.add(font, TextShow::Quote(value.2.as_bytes()));
            }
            _ => {}
        }
    }

    /// Retain decoder save/restore for the binding active at this boundary.
    fn state(&mut self, font: Option<usize>, command: &str) {
        if let Some(font) = font {
            self.batches[font].content.push_str(command);
        }
    }

    /// Enter a Form's resource scope unless its stream is already executing.
    fn form(&mut self, resources: &Resources<'a>, name: &Name<'_>, font: Option<usize>) {
        let Some(stream) = resources.get_x_object(name) else {
            return;
        };
        if stream.dict().get::<Name<'_>>(b"Subtype").as_deref() != Some(b"Form")
            || stream.dict().get::<[f32; 4]>(b"BBox").is_none()
        {
            return;
        }
        let Ok(decoded) = stream.decoded() else {
            return;
        };
        let child = stream
            .dict()
            .get::<Dict<'_>>(b"Resources")
            .map_or_else(|| resources.clone(), Resources::new);
        let identity = stream.obj_id();
        if !self.active_forms.insert(identity) {
            return;
        }
        self.state(font, "q\n");
        self.walk(TypedIter::new(decoded.as_ref()), &child, font);
        self.state(font, "Q\n");
        self.active_forms.remove(&identity);
    }
}

/// Match the native graphics-state Font array's dictionary and size requirements.
fn graphics_state_has_font(resources: &Resources<'_>, name: &Name<'_>) -> bool {
    resources
        .get_ext_g_state(name)
        .and_then(|state| state.get::<Array<'_>>(b"Font"))
        .is_some_and(|array| {
            let mut values = array.iter::<Object<'_>>();
            values.next().and_then(Object::into_dict).is_some()
                && values.next().and_then(Object::into_number).is_some()
        })
}

/// Encode raw string bytes as PDF hexadecimal syntax, never diagnostic text.
fn write_hex(output: &mut String, bytes: &[u8]) {
    output.push('<');
    for byte in bytes {
        let _ = write!(output, "{byte:02X}");
    }
    output.push_str("> ");
}

/// Collect decoded Unicode in the original logical show's output slot.
#[derive(Default)]
struct TextDevice {
    /// Owned page-order runs.
    strings: Vec<String>,
    /// Current batch's page-order slots.
    slots: Vec<usize>,
    /// Next marker's index within the batch.
    next: usize,
    /// Slot selected by the latest marker.
    current: Option<usize>,
}
impl<'a> Device<'a> for TextDevice {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_image(&mut self, _: Image<'a, '_>, _: ImageDrawProps<'a>) {}
    fn pop_clip(&mut self) {}
    fn pop_transparency_group(&mut self) {}
    fn begin_marked_content(&mut self, _: &[u8], _: Option<i32>) {
        self.current = self.slots.get(self.next).copied();
        self.next += 1;
    }
    fn draw_glyph_run(&mut self, run: &GlyphRun<'_, 'a>, _: DrawProps<'a>, _: &DrawMode) {
        let Some(result) = self.current.and_then(|index| self.strings.get_mut(index)) else {
            return;
        };
        for value in run.glyphs().iter().filter_map(|glyph| glyph.as_unicode()) {
            match value {
                BfString::Char(c) => result.push(c),
                BfString::String(value) => result.push_str(&value),
            }
        }
    }
}

/// Decode selected shows once, independently of page painting and visibility.
pub(super) fn get_text_content<'a>(page: &Page<'a>, cache: &InterpreterCache<'a>) -> Vec<String> {
    let mut shows = Shows::default();
    shows.walk(page.typed_operations(), page.resources(), None);
    let mut device = TextDevice {
        strings: vec![String::new(); shows.count],
        ..Default::default()
    };
    let mut context = Context::new(
        Affine::IDENTITY,
        Rect::new(0.0, 0.0, 1.0, 1.0),
        cache,
        page.xref(),
        InterpreterSettings {
            render_annotations: false,
            ..Default::default()
        },
    );
    for batch in shows.batches {
        device.slots = batch.slots;
        device.next = 0;
        device.current = None;
        let encoded = format!("BT 3 Tr\n{} ET", batch.content);
        interpret(
            TypedIter::new(encoded.as_bytes()),
            &batch.resources,
            &mut context,
            &mut device,
        );
    }
    device.strings
}
