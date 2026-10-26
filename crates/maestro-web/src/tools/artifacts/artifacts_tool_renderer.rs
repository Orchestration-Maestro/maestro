//! Artifact command selection over borrowed raw and native operands.
use super::artifact_pill::Pill;
use crate::{
    components::{console_block::ConsoleVariant, diff::DiffProps},
    tools::{
        renderer_registry::get_text_output,
        renderers::{
            card_content::{Caption, CardOutput},
            collapsible_card::{ArtifactTemplate, Body, CollapsibleCard, Phase},
            default_renderer::status,
        },
        types::{ToolRenderResult, ToolRenderer, ToolResultMessageView, ToolUiError, json_params},
    },
};
use maestro_app::{
    JsonObject,
    browser::tools::artifacts::{ArtifactCommand, ArtifactsParams, LocalArtifactsPanel},
};
use serde_json::Value;
use std::{borrow::Cow, rc::Rc};

/// Artifact cards capturing an optional application panel during preparation.
pub struct ArtifactsToolRenderer {
    /// Panel used by subsequently prepared filename actions.
    pub artifacts_panel: Option<Rc<LocalArtifactsPanel>>,
}
impl ArtifactsToolRenderer {
    /// Bind future cards to the supplied panel; no global registration is installed.
    #[must_use]
    pub fn new(artifacts_panel: Option<Rc<LocalArtifactsPanel>>) -> Self {
        Self { artifacts_panel }
    }
}
/// Execution phase, distinct from the presentation of a nonstreaming pending call.
#[derive(Clone, Copy)]
enum Outcome {
    /// Arguments are available without an execution result.
    Pending,
    /// Execution returned successfully.
    Success,
    /// Execution returned a tool error.
    Error,
}
/// Borrowed rendering ingress, independent of executable-tool validation.
#[derive(Clone, Copy)]
enum Input<'a> {
    /// Raw partial model object.
    Raw(&'a JsonObject),
    /// Validated native arguments.
    Native(&'a ArtifactsParams),
}
/// One selected raw or native field, without converting a whole record.
#[derive(Clone, Copy)]
enum Field<'a> {
    /// JSON boundary value, including null and malformed field types.
    Json(&'a Value),
    /// Native literal text.
    Text(&'a str),
}
impl<'a> Input<'a> {
    /// Read only a field reached by the selected command branch.
    fn field(self, key: &str) -> Option<Field<'a>> {
        match self {
            Self::Raw(object) => object.get(key).map(Field::Json),
            Self::Native(params) => match key {
                "command" => Some(Field::Text(match params.command {
                    ArtifactCommand::Create => "create",
                    ArtifactCommand::Update => "update",
                    ArtifactCommand::Rewrite => "rewrite",
                    ArtifactCommand::Get => "get",
                    ArtifactCommand::Delete => "delete",
                    ArtifactCommand::Logs => "logs",
                })),
                "filename" => Some(Field::Text(&params.filename)),
                "content" => params.content.as_deref().map(Field::Text),
                "old_str" => params.old_str.as_deref().map(Field::Text),
                "new_str" => params.new_str.as_deref().map(Field::Text),
                _ => None,
            },
        }
    }
}
impl<'a> Field<'a> {
    /// Preserve literal string identity separately from display-only projection.
    fn text(self) -> Option<&'a str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Json(value) => value.as_str(),
        }
    }
    /// Whole-value truth follows the shared boundary owner.
    fn truthy(self) -> bool {
        match self {
            Self::Text(text) => !text.is_empty(),
            Self::Json(value) => maestro_app::is_truthy(value),
        }
    }
    /// Selected nullish operands are empty; other non-strings fail at their consumer.
    fn operand(self) -> Result<&'a str, ToolUiError> {
        if matches!(self, Self::Json(Value::Null)) {
            return Ok("");
        }
        self.text()
            .ok_or_else(|| ToolUiError::Presentation("Artifact operand must be text".into()))
    }
    /// Shared readable projection, never another JSON writer.
    fn display(self) -> Result<Option<Cow<'a, str>>, ToolUiError> {
        match self {
            Self::Text(text) => Ok((!text.is_empty()).then_some(Cow::Borrowed(text))),
            Self::Json(value) => maestro_app::display_truthy(Some(value))
                .map_err(|error| ToolUiError::Presentation(error.to_string())),
        }
    }
}
impl<D, C: CardOutput> ToolRenderer<JsonObject, D, C> for ArtifactsToolRenderer {
    fn render(
        &self,
        params: Option<&JsonObject>,
        result: Option<ToolResultMessageView<'_, D>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult<C>, ToolUiError> {
        self.prepare(params.map(Input::Raw), result.as_ref(), is_streaming)
    }
}
impl<D, C: CardOutput> ToolRenderer<Value, D, C> for ArtifactsToolRenderer {
    fn render(
        &self,
        params: Option<&Value>,
        result: Option<ToolResultMessageView<'_, D>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult<C>, ToolUiError> {
        let empty = JsonObject::new();
        self.prepare(
            json_params(params, &empty).map(Input::Raw),
            result.as_ref(),
            is_streaming,
        )
    }
}
impl<D, C: CardOutput> ToolRenderer<ArtifactsParams, D, C> for ArtifactsToolRenderer {
    fn render(
        &self,
        params: Option<&ArtifactsParams>,
        result: Option<ToolResultMessageView<'_, D>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult<C>, ToolUiError> {
        self.prepare(params.map(Input::Native), result.as_ref(), is_streaming)
    }
}
impl ArtifactsToolRenderer {
    /// Select a template and own only the fields that survive that selection.
    fn prepare<D, C: CardOutput>(
        &self,
        params: Option<Input<'_>>,
        result: Option<&ToolResultMessageView<'_, D>>,
        streaming: bool,
    ) -> Result<ToolRenderResult<C>, ToolUiError> {
        let command_field = params.and_then(|params| params.field("command"));
        let command = command_field.and_then(Field::text).unwrap_or_default();
        let outcome = match result {
            Some(result) if result.is_error() => Outcome::Error,
            Some(_) => Outcome::Success,
            None => Outcome::Pending,
        };
        let error = matches!(outcome, Outcome::Error);
        let complete = matches!(outcome, Outcome::Success);
        let preparing = !error
            && (params.is_none() || (!complete && !command_field.is_some_and(Field::truthy)));
        let template = template(command, preparing, outcome);
        let mut card = CollapsibleCard {
            state: status(result, streaming),
            glyph: "FileCode2",
            caption: Caption::Artifact(
                if preparing {
                    "Preparing artifact..."
                } else {
                    label(command, complete)
                },
                None,
            ),
            body: None,
            attachments: Vec::new(),
            plain_phase: Some(Phase::Artifact(template)),
            error: None,
        };
        if preparing {
            return Ok(card.result::<Self>());
        }
        let edit = matches!(command, "create" | "rewrite" | "update");
        let output = get_text_output(result);
        if error && !edit {
            card.error = Some(output);
            return Ok(card.result::<Self>());
        }
        let filename = params.and_then(|params| params.field("filename"));
        if let Some(display) = filename.map(Field::display).transpose()?.flatten() {
            card.caption = Caption::Artifact(
                label(command, complete),
                Some(Pill {
                    text: display.into_owned(),
                    filename: filename.and_then(Field::text).map(str::to_owned),
                    panel: self.artifacts_panel.clone(),
                }),
            );
        }
        if edit || matches!(command, "get" | "logs") {
            card.body = Some(body(
                params,
                (command, filename),
                outcome,
                output,
                template,
            )?);
        }
        Ok(card.result::<Self>())
    }
}
/// Fixed caption keys; caller-authored filenames and content never enter localization.
fn label(command: &str, complete: bool) -> &'static str {
    match (command, complete) {
        ("create", false) => "Creating artifact",
        ("create", true) => "Created artifact",
        ("update", false) => "Updating artifact",
        ("update", true) => "Updated artifact",
        ("rewrite", false) => "Rewriting artifact",
        ("rewrite", true) => "Rewrote artifact",
        ("get", false) => "Getting artifact",
        ("get", true) => "Got artifact",
        ("logs", false) => "Getting logs",
        ("logs", true) => "Got logs",
        ("delete", false) => "Deleting artifact",
        ("delete", true) => "Deleted artifact",
        (_, false) => "Processing artifact",
        (_, true) => "Processed artifact",
    }
}
/// Subtree keys mirror the independently retained command templates.
fn template(command: &str, preparing: bool, outcome: Outcome) -> ArtifactTemplate {
    use ArtifactTemplate as T;
    let complete = matches!(outcome, Outcome::Success);
    let error = matches!(outcome, Outcome::Error);
    if preparing {
        return T::Preparing;
    }
    if error {
        return if matches!(command, "create" | "rewrite" | "update") {
            T::ErrorEdit
        } else {
            T::ErrorPlain
        };
    }
    match (command, complete) {
        ("create" | "rewrite", false) => T::PendingCode,
        ("create" | "rewrite", true) => T::ResultCode,
        ("update", false) => T::PendingDiff,
        ("update", true) => T::ResultDiff,
        ("get" | "logs", false) => T::PendingEmpty,
        ("get", true) => T::ResultGet,
        ("logs", true) => T::ResultLogs,
        (_, false) => T::PendingPlain,
        (_, true) => T::ResultPlain,
    }
}
/// Select mutually exclusive body operands without validating discarded fields.
fn body<C: CardOutput>(
    params: Option<Input<'_>>,
    (command, filename): (&str, Option<Field<'_>>),
    outcome: Outcome,
    output: String,
    template: ArtifactTemplate,
) -> Result<Body<C>, ToolUiError> {
    let complete = matches!(outcome, Outcome::Success);
    let error = matches!(outcome, Outcome::Error);
    let mut body = Body {
        phase: Phase::Artifact(template),
        url: None,
        code: None,
        console: None,
        diff: None,
        error: None,
        fallback: None,
    };
    match command {
        "update" => body.diff = replacements(params, complete)?,
        "create" | "rewrite" => {
            if let Some(content) = params
                .and_then(|params| params.field("content"))
                .map(Field::display)
                .transpose()?
                .flatten()
            {
                body.code = Some((content.into_owned(), language(filename)?));
            }
        }
        "get" if complete => {
            body.code = Some((output, language(filename)?));
            body.fallback = Some("(no output)");
            return Ok(body);
        }
        "logs" if complete => {
            body.console = Some((output, ConsoleVariant::Default));
            body.fallback = Some("(no output)");
            return Ok(body);
        }
        _ => {}
    }
    if matches!(command, "create" | "rewrite" | "update") && (complete || error) {
        let html = filename
            .map(Field::operand)
            .transpose()?
            .is_some_and(|name| name.as_bytes().ends_with(b".html"));
        if error {
            body.fallback = Some("An error occurred");
            if html {
                body.console = Some((output, ConsoleVariant::Error));
            } else {
                body.error = Some(output);
            }
        } else if html && !output.is_empty() {
            body.console = Some((output, ConsoleVariant::Default));
        }
    }
    Ok(body)
}
/// Reached replacement validation differs between pending/error and success.
fn replacements<C: CardOutput>(
    params: Option<Input<'_>>,
    complete: bool,
) -> Result<Option<C>, ToolUiError> {
    let old = params.and_then(|params| params.field("old_str"));
    let new = params.and_then(|params| params.field("new_str"));
    if !complete && (old.is_none() || new.is_none()) {
        return Ok(None);
    }
    let operand = |field: Option<Field<'_>>| {
        field
            .filter(|value| !complete || value.truthy())
            .map(Field::operand)
            .transpose()
            .map(|text| text.unwrap_or_default().to_owned())
    };
    Ok(Some(C::diff(DiffProps {
        old_text: operand(old)?,
        new_text: operand(new)?,
        title: None,
        class_name: String::new(),
    })))
}
/// Literal final dot-segment classification, never filesystem normalization.
fn language(filename: Option<Field<'_>>) -> Result<&'static str, ToolUiError> {
    let Some(filename) = filename.filter(|field| field.truthy()) else {
        return Ok("text");
    };
    Ok(match super::extension(filename.operand()?).as_str() {
        "js" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        "html" => "html",
        "css" => "css",
        "scss" => "scss",
        "json" => "json",
        "py" => "python",
        "md" => "markdown",
        "svg" | "xml" => "xml",
        "yaml" | "yml" => "yaml",
        "sh" | "bash" => "bash",
        "sql" => "sql",
        "java" => "java",
        "c" => "c",
        "cpp" => "cpp",
        "cs" => "csharp",
        "go" => "go",
        "rs" => "rust",
        "php" => "php",
        "rb" => "ruby",
        "swift" => "swift",
        "kt" => "kotlin",
        "r" => "r",
        _ => "text",
    })
}
