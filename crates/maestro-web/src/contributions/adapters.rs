//! Module element and isolated frame adapters behind the shared contracts.
use super::{ContributionFailure, ContributionView};
use crate::{
    MessageRenderer,
    browser::attachment_content::browser::{element, host_error},
    dom::{DomContent, DomView},
    tools::types::{ToolRenderResult, ToolRenderer, ToolResultMessageView, ToolUiError},
};
use maestro_app::{AgentMessage, CustomAgentMessages, JsonObject};
use serde_json::{Value, json};
use wasm_bindgen::JsCast as _;
use web_sys::{Element, Node};

/// Property carrying a module element's serializable input.
const INPUT_PROPERTY: &str = "maestroInput";
/// Event announcing a changed input on a connected module element.
const INPUT_EVENT: &str = "maestro-input";
/// A module element created by tag and fed explicit serializable input.
pub(super) struct ElementContent {
    /// Element tag defined by the module.
    pub(super) tag: String,
    /// Serializable input.
    pub(super) input: Value,
}
impl DomContent for ElementContent {
    fn mount(&self, view: &DomView) -> Result<Node, ToolUiError> {
        let created = view
            .document
            .create_element(&self.tag)
            .map_err(|error| host_error(&error))?;
        set_input(&created, &self.input)?;
        Ok(created.into())
    }
    fn update(&self, root: &Node, _: &DomView) -> Result<(), ToolUiError> {
        let mounted: &Element = root.unchecked_ref();
        set_input(mounted, &self.input)?;
        let event = web_sys::Event::new(INPUT_EVENT).map_err(|error| host_error(&error))?;
        mounted
            .dispatch_event(&event)
            .map_err(|error| host_error(&error))?;
        Ok(())
    }
}
/// Store the input as a plain browser value.
fn set_input(target: &Element, input: &Value) -> Result<(), ToolUiError> {
    let text =
        serde_json::to_string(input).map_err(|e| ToolUiError::Presentation(e.to_string()))?;
    let value = js_sys::JSON::parse(&text).map_err(|error| host_error(&error))?;
    js_sys::Reflect::set(target, &INPUT_PROPERTY.into(), &value)
        .map_err(|error| host_error(&error))?;
    Ok(())
}
/// A visible failure in place of the contribution.
pub(super) struct FailureContent(pub(super) ContributionFailure);
impl DomContent for FailureContent {
    fn mount(&self, view: &DomView) -> Result<Node, ToolUiError> {
        let failure = element(&view.document, "div", "maestro-contribution-failure")?;
        failure
            .set_attribute("role", "alert")
            .map_err(|error| host_error(&error))?;
        self.update(&failure, view)?;
        Ok(failure.into())
    }
    fn update(&self, root: &Node, _: &DomView) -> Result<(), ToolUiError> {
        root.set_text_content(Some(&self.0.to_string()));
        Ok(())
    }
}
/// Module element rendering one tool.
pub(super) struct ElementToolRenderer {
    /// Element tag.
    pub(super) tag: String,
    /// Declaration, for required APIs.
    pub(super) descriptor: super::ContributionDescriptor,
}
impl ToolRenderer<JsonObject, Value, Box<dyn DomContent>> for ElementToolRenderer {
    fn render(
        &self,
        params: Option<&JsonObject>,
        result: Option<ToolResultMessageView<'_, Value>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult<Box<dyn DomContent>>, ToolUiError> {
        let content: Box<dyn DomContent> = match super::admit(&self.descriptor, None) {
            Err(failure) => Box::new(FailureContent(failure)),
            Ok(()) => Box::new(ElementContent {
                tag: self.tag.clone(),
                input: json!({
                    "params": params,
                    "result": result.as_ref().map(result_json),
                    "isStreaming": is_streaming,
                }),
            }),
        };
        Ok(ToolRenderResult {
            content,
            is_custom: true,
        })
    }
}
/// The result message's serializable form.
fn result_json(result: &ToolResultMessageView<'_, Value>) -> Value {
    json!({
        "role": result.role(),
        "toolCallId": result.tool_call_id(),
        "toolName": result.tool_name(),
        "content": result.content(),
        "details": result.details(),
        "isError": result.is_error(),
        "timestamp": result.timestamp(),
    })
}
/// Module element rendering one message role.
pub(super) struct ElementMessageRenderer {
    /// Element tag.
    pub(super) tag: String,
}
impl<C: CustomAgentMessages + serde::Serialize> MessageRenderer<C, Box<dyn DomContent>>
    for ElementMessageRenderer
{
    fn render(&self, message: &AgentMessage<C>) -> Result<Box<dyn DomContent>, ToolUiError> {
        let message =
            serde_json::to_value(message).map_err(|e| ToolUiError::Presentation(e.to_string()))?;
        Ok(Box::new(ElementContent {
            tag: self.tag.clone(),
            input: json!({ "message": message }),
        }))
    }
}
/// Module element showing a slotted contribution.
pub(super) struct ElementView {
    /// Element tag.
    pub(super) tag: String,
}
impl ContributionView for ElementView {
    fn content(&self, input: &Value) -> Box<dyn DomContent> {
        Box::new(ElementContent {
            tag: self.tag.clone(),
            input: input.clone(),
        })
    }
}
/// Isolated frame showing an untrusted package document.
pub(super) struct FrameView {
    /// Contribution identity used as the frame's registered source.
    pub(super) id: String,
    /// Fetched document text.
    pub(super) document: String,
    /// Isolated-frame document URL selection.
    pub(super) url: Option<maestro_app::SandboxUrlProvider>,
}
impl ContributionView for FrameView {
    fn content(&self, input: &Value) -> Box<dyn DomContent> {
        Box::new(FrameContent {
            id: self.id.clone(),
            document: self.document.clone(),
            input: input.clone(),
            url: self.url.clone(),
        })
    }
}
/// One isolated frame and the input injected into it.
struct FrameContent {
    /// Registered frame source identity.
    id: String,
    /// Document text.
    document: String,
    /// Serializable input exposed inside the frame.
    input: Value,
    /// Isolated-frame document URL selection.
    url: Option<maestro_app::SandboxUrlProvider>,
}
impl DomContent for FrameContent {
    fn mount(&self, view: &DomView) -> Result<Node, ToolUiError> {
        let parent: web_sys::HtmlElement = view
            .document
            .create_element("div")
            .map_err(|error| host_error(&error))?
            .unchecked_into();
        let presentation = view
            .pack
            .presentation(view.mode)
            .map_err(|e| ToolUiError::Presentation(e.to_string()))?;
        let frame = crate::SandboxIframe::new(parent.clone(), &presentation)
            .map_err(|e| ToolUiError::Presentation(e.message))?;
        frame
            .sandbox_url_provider
            .borrow_mut()
            .clone_from(&self.url);
        let provider: maestro_app::SharedHandle<
            dyn maestro_app::SandboxRuntimeProvider<
                    crate::browser::sandbox_value::BrowserSandboxValue,
                >,
        > = maestro_app::SharedHandle::new(InputProvider(self.input.clone()));
        frame
            .load_content(&self.id, &self.document, vec![provider], Vec::new())
            .map_err(|e| ToolUiError::Presentation(e.message))?;
        FRAMES.with(|all| all.borrow_mut().push((parent.clone().into(), frame)));
        Ok(parent.into())
    }
    fn update(&self, _: &Node, _: &DomView) -> Result<(), ToolUiError> {
        Ok(())
    }
}
thread_local! {
    /// Mounted frames kept until their container leaves the slot.
    static FRAMES: std::cell::RefCell<Vec<(Node, crate::SandboxIframe)>> = const { std::cell::RefCell::new(Vec::new()) };
}
/// Release the frame mounted in `root`.
pub(super) fn release_frame(root: &Node) {
    let removed = FRAMES.with(|all| {
        let mut all = all.borrow_mut();
        let index = all.iter().position(|(node, _)| node == root)?;
        Some(all.remove(index))
    });
    if let Some((_, frame)) = removed {
        let _ = frame.remove();
    }
}
/// Injects the contribution input as a frame global.
struct InputProvider(Value);
impl maestro_app::SandboxRuntimeProvider<crate::browser::sandbox_value::BrowserSandboxValue>
    for InputProvider
{
    fn get_data(
        &self,
    ) -> Result<
        indexmap::IndexMap<String, crate::browser::sandbox_value::BrowserSandboxValue>,
        maestro_app::DiagnosticErrorInfo,
    > {
        let text = serde_json::to_string(&self.0)
            .map_err(|e| maestro_app::DiagnosticErrorInfo::error(e.to_string()))?;
        let value = js_sys::JSON::parse(&text)
            .map_err(|_| maestro_app::DiagnosticErrorInfo::error("Invalid input"))?;
        Ok(indexmap::IndexMap::from([(
            INPUT_PROPERTY.to_owned(),
            crate::browser::sandbox_value::BrowserSandboxValue(value),
        )]))
    }
    fn get_runtime(&self) -> Result<String, maestro_app::DiagnosticErrorInfo> {
        Ok("function () {}".into())
    }
    fn get_description(&self) -> Result<String, maestro_app::DiagnosticErrorInfo> {
        Ok(String::new())
    }
}
