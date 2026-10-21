//! Editable browser author fixture built from page elements.

/// Mount the prepared stylesheet and mark beside the authored input.
fn start() -> Result<(), wasm_bindgen::JsValue> {
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
    Ok(())
}

/// Start the author fixture, exposing a native browser error on startup failure.
fn main() {
    if let Err(error) = start() {
        wasm_bindgen::throw_val(error);
    }
}
