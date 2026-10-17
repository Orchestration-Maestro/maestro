//! Editable browser author fixture using prepared shared presentation.
use eframe::egui;
mod editor;
use wasm_bindgen::JsCast;

/// Editable state retained by the framework's application owner.
#[derive(Default)]
struct AuthorInput(String);

impl eframe::App for AuthorInput {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        ui.label(include_str!(concat!(
            env!("MAESTRO_BROWSER_INPUTS"),
            "/wordmark.txt"
        )));
        editor::show(ui, &mut self.0);
        ui.label(&self.0);
    }
}

/// Prepare only fixture-owned platform presentation.
fn prepare(context: &egui::Context) {
    let mut fonts = egui::FontDefinitions::empty();
    for (name, bytes) in [
        (
            "Barlow",
            include_bytes!("../../../../../../assets/brand/fonts/Barlow-Regular.ttf").as_slice(),
        ),
        (
            "Barlow Condensed",
            include_bytes!("../../../../../../assets/brand/fonts/BarlowCondensed-Bold.ttf")
                .as_slice(),
        ),
        (
            "Barlow Condensed 600",
            include_bytes!("../../../../../../assets/brand/fonts/BarlowCondensed-SemiBold.ttf")
                .as_slice(),
        ),
        (
            "Space Grotesk",
            include_bytes!("../../../../../../assets/brand/fonts/SpaceGrotesk.ttf").as_slice(),
        ),
        (
            "JetBrains Mono",
            include_bytes!(
                "../../../../../../assets/brand/fonts/JetBrainsMono-official-Regular.ttf"
            )
            .as_slice(),
        ),
        (
            "Noto Sans Symbols 2",
            include_bytes!("../../../../../../assets/brand/fonts/NotoSansSymbols2-Regular.ttf")
                .as_slice(),
        ),
    ] {
        fonts
            .font_data
            .insert(name.into(), egui::FontData::from_static(bytes).into());
    }
    let body = include_str!(concat!(env!("MAESTRO_BROWSER_INPUTS"), "/body.txt"));
    let chain = [body, "JetBrains Mono", "Noto Sans Symbols 2"]
        .map(str::to_owned)
        .to_vec();
    fonts.families.insert(egui::FontFamily::Proportional, chain);
    fonts.families.insert(
        egui::FontFamily::Monospace,
        vec!["JetBrains Mono".into(), "Noto Sans Symbols 2".into()],
    );
    context.set_fonts(fonts);
}

/// Apply the fixture widget roles from the shared projection.
fn style(context: &egui::Context) -> std::io::Result<()> {
    let text = color(include_str!(concat!(
        env!("MAESTRO_BROWSER_INPUTS"),
        "/text.txt"
    )))?;
    let background = color(include_str!(concat!(
        env!("MAESTRO_BROWSER_INPUTS"),
        "/background.txt"
    )))?;
    let panel = color(include_str!(concat!(
        env!("MAESTRO_BROWSER_INPUTS"),
        "/panel.txt"
    )))?;
    let border = color(include_str!(concat!(
        env!("MAESTRO_BROWSER_INPUTS"),
        "/border.txt"
    )))?;
    let accent = color(include_str!(concat!(
        env!("MAESTRO_BROWSER_INPUTS"),
        "/accent.txt"
    )))?;
    let on_accent = color(include_str!(concat!(
        env!("MAESTRO_BROWSER_INPUTS"),
        "/on-accent.txt"
    )))?;
    context.set_visuals(editor::visuals([
        text, background, panel, border, accent, on_accent,
    ]));
    let size = include_str!(concat!(env!("MAESTRO_BROWSER_INPUTS"), "/body-size.txt"))
        .parse()
        .map_err(std::io::Error::other)?;
    context
        .global_style_mut(|style| style.override_font_id = Some(egui::FontId::proportional(size)));
    Ok(())
}

/// Mount embedded CSS and the resolved mark before launching the framework.
async fn start() -> Result<(), wasm_bindgen::JsValue> {
    let document = web_sys::window()
        .ok_or("missing window")?
        .document()
        .ok_or("missing document")?;
    let stylesheet = document.create_element("style")?;
    stylesheet.set_text_content(Some(include_str!(concat!(
        env!("MAESTRO_BROWSER_INPUTS"),
        "/brand.css"
    ))));
    document
        .body()
        .ok_or("missing body")?
        .append_child(&stylesheet)?;
    document
        .get_element_by_id("maestro-mark")
        .ok_or("missing mark")?
        .set_inner_html(include_str!(concat!(
            env!("MAESTRO_BROWSER_INPUTS"),
            "/mark.svg"
        )));
    let canvas = document
        .get_element_by_id("maestro-canvas")
        .ok_or("missing canvas")?
        .dyn_into()?;
    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|cc| {
                prepare(&cc.egui_ctx);
                style(&cc.egui_ctx)?;
                Ok(Box::<AuthorInput>::default())
            }),
        )
        .await
}

/// Start the author fixture, exposing a native browser error on startup failure.
fn main() {
    wasm_bindgen_futures::spawn_local(async {
        if let Err(error) = start().await {
            wasm_bindgen::throw_val(error);
        }
    });
}

/// Convert an admitted hex color through the toolkit.
fn color(text: &str) -> std::io::Result<egui::Color32> {
    egui::Color32::from_hex(text).map_err(|error| std::io::Error::other(format!("{error:?}")))
}
