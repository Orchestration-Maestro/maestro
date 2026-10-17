//! Native frame construction, validation and literal error documents.
use super::{
    BrowserSandboxValue, DiagnosticErrorInfo, MessageConsumer, Owner, Provider, diagnostic,
};
use crate::components::sandboxed_iframe::PrepareHtmlOptions;
use js_sys::Reflect;
use maestro_app::SharedHandle;
use maestro_app::browser::components::sandbox::console_runtime_provider::ConsoleRuntimeProvider;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue};
use web_sys::{Document, DomParser, HtmlIFrameElement, SupportedType};

impl Owner {
    /// Acquire the context, prepared document, frame and subscription before mounting.
    pub(super) fn load(
        &self,
        id: &str,
        code: &str,
        providers: Vec<Provider>,
        consumers: Vec<SharedHandle<dyn MessageConsumer<BrowserSandboxValue>>>,
    ) -> Result<(), DiagnosticErrorInfo> {
        let providers: Vec<Provider> = std::iter::once(SharedHandle::new(ConsoleRuntimeProvider::<
            BrowserSandboxValue,
        >::new()) as Provider)
        .chain(providers)
        .collect();
        crate::RUNTIME_MESSAGE_ROUTER.with(|router| {
            router.register_sandbox(
                id.into(),
                Rc::new(RefCell::new(providers.clone())),
                consumers,
            )
        })?;
        let html = self.prepare_html_document(
            id,
            code,
            &providers,
            Some(PrepareHtmlOptions {
                is_html_artifact: true,
                is_standalone: false,
            }),
        )?;
        self.mount(id, html, Mount::Loading)
    }
    /// Validate and mount through the same retained frame and one-shot handshake.
    pub(super) fn mount(
        &self,
        id: &str,
        html: String,
        mode: Mount,
    ) -> Result<(), DiagnosticErrorInfo> {
        let error = validate(&html);
        if let (Mount::Execution, Some(error)) = (mode, &error) {
            return Err(DiagnosticErrorInfo::error(format!(
                "HTML validation failed: {error}"
            )));
        }
        let document = self
            .parent
            .owner_document()
            .ok_or_else(|| DiagnosticErrorInfo::error("Missing parent document"))?;
        let frame: HtmlIFrameElement = document
            .create_element("iframe")
            .map_err(|error| diagnostic(&error))?
            .dyn_into()
            .map_err(|error| diagnostic(&error))?;
        self.state.borrow_mut().frame = Some(frame.clone());
        let url = self.url.borrow().clone();
        let hidden = matches!(mode, Mount::Execution) && url.is_none();
        frame
            .set_attribute(
                "style",
                if hidden {
                    "width:100%;height:100%;border:none;display:none;"
                } else {
                    "width:100%;height:100%;border:none;"
                },
            )
            .map_err(|error| diagnostic(&error))?;
        if let Some(error) = error {
            frame.set_srcdoc(
                &validation_document(&document, &self.error_style, &error)
                    .map_err(|error| diagnostic(&error))?,
            );
        } else {
            frame
                .set_attribute("sandbox", "allow-scripts allow-modals")
                .map_err(|error| diagnostic(&error))?;
            if let Some(provider) = url {
                let url = provider()?;
                frame.set_src(&url);
                let mut state = self.state.borrow_mut();
                state.extension = true;
                state.pending = Some(html);
            } else {
                frame.set_srcdoc(&html);
            }
        }
        crate::RUNTIME_MESSAGE_ROUTER.with(|router| router.set_sandbox_iframe(id, frame.clone()));
        self.subscribe().map_err(|error| diagnostic(&error))?;
        self.parent
            .append_child(&frame)
            .map_err(|error| diagnostic(&error))?;
        Ok(())
    }
}
/// Loading paints validation failures; execution rejects them.
#[derive(Clone, Copy)]
pub(super) enum Mount {
    /// Caller-visible HTML content.
    Loading,
    /// Hidden or SDK-authored execution.
    Execution,
}
/// Native text/html recovery is accepted; explicit parser errors retain their text.
fn validate(html: &str) -> Option<String> {
    let result = DomParser::new()
        .and_then(|parser| parser.parse_from_string(html, SupportedType::TextHtml))
        .and_then(|document| document.query_selector("parsererror"));
    match result {
        Ok(None) => None,
        Ok(Some(element)) => Some(
            element
                .text_content()
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| "Unknown parse error".into()),
        ),
        Err(error) => Some(
            Reflect::get(&error, &"message".into())
                .ok()
                .and_then(|message| message.as_string())
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| "Unknown validation error".into()),
        ),
    }
}
/// Serialize native literal text, not interpolated active HTML, in the selected host style.
fn validation_document(document: &Document, css: &str, error: &str) -> Result<String, JsValue> {
    let html = document.create_element("html")?;
    let style = document.create_element("style")?;
    style.set_text_content(Some(&format!(":root{{{css}}}body{{font-family:var(--maestro-font-mono);padding:20px;background:var(--maestro-color-background);color:var(--maestro-color-text)}}h3{{color:var(--maestro-color-error)}}pre{{font-family:inherit;background:var(--maestro-color-panel);padding:10px;border-radius:4px;overflow-x:auto;white-space:pre-wrap}}")));
    let body = document.create_element("body")?;
    let heading = document.create_element("h3")?;
    heading.set_text_content(Some("HTML Validation Error"));
    let pre = document.create_element("pre")?;
    pre.set_text_content(Some(error));
    body.append_child(&heading)?;
    body.append_child(&pre)?;
    html.append_child(&style)?;
    html.append_child(&body)?;
    Ok(html.outer_html())
}
