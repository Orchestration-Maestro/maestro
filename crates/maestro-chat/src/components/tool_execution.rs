//! Retained transcript composition over caller-selected tool renderer bindings.
use maestro_app::{
    DiagnosticErrorInfo, ToolRenderContext, ToolRenderEnvironment, ToolRenderPhase,
    ToolRenderResultLike, ToolRenderResultOptions, ToolRenderShell, ToolRendererBinding,
    ToolRendererState, UserBlock, expand_hint, get_text_output, pretty_json,
};
use maestro_theme::{Theme, ThemeBg, ThemeColor};
use maestro_tui::{Box as Frame, Component, Container, Spacer, Text, tui::ComponentHandle};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};

mod images;

/// Lines a collapsed fallback result shows before its expand hint.
const FALLBACK_PREVIEW_LINES: usize = 10;

/// Initial image display policies.
pub struct ToolExecutionOptions {
    /// Whether supported inline images are visible.
    pub show_images: bool,
    /// Maximum image width; the image boundary floors and clamps it to integral cells.
    pub image_width_cells: f64,
}
impl Default for ToolExecutionOptions {
    fn default() -> Self {
        Self {
            show_images: true,
            image_width_cells: 60.0,
        }
    }
}
/// Owned inputs for a retained tool row.
pub struct ToolExecutionParams {
    /// Displayed tool name.
    pub tool_name: String,
    /// Stable call identifier.
    pub tool_call_id: String,
    /// Raw arguments, including explicit null and scalar roots.
    pub args: Option<Value>,
    /// Initial image policies.
    pub options: ToolExecutionOptions,
    /// Optional custom renderer projection.
    pub tool_definition: Option<ToolRendererBinding>,
    /// Optional native renderer projection selected by the caller.
    pub built_in_tool_definition: Option<ToolRendererBinding>,
    /// Existing rendering effects.
    pub environment: Rc<ToolRenderEnvironment>,
    /// Session working directory.
    pub cwd: String,
    /// Render request that must not strongly retain its writer.
    pub request_render: Rc<dyn Fn()>,
}
/// Current result without execution-only metadata.
pub struct ToolExecutionResult {
    /// Ordered text and image blocks.
    pub content: Vec<UserBlock>,
    /// Raw persisted details, preserving absent versus null.
    pub details: Option<Value>,
    /// Whether execution failed, independently of streaming.
    pub is_error: bool,
}
impl ToolExecutionResult {
    /// Borrow the existing renderer boundary without narrowing details.
    fn borrowed(&self) -> ToolRenderResultLike<'_, Value> {
        ToolRenderResultLike {
            content: &self.content,
            details: self.details.as_ref(),
            terminate: None,
            is_error: None,
        }
    }
}
/// Editable outer children with one retained composition owner.
pub struct ToolExecutionComponent(Rc<Row>);
/// Exactly one chosen shell, preserving edits to the outer container.
enum Shell {
    /// Host-provided padded background.
    Default(Rc<Frame>),
    /// Tool-provided framing.
    SelfRendered(Rc<Container>),
    /// Unknown tool's raw JSON and text.
    Generic(Rc<Text>),
}
/// Mutable observations are separate from callback and component owners.
struct Row {
    /// Inherited outer child collection.
    container: Container,
    /// Chosen inner shell, even after removal from outer children.
    shell: Shell,
    /// Stable constructor inputs; mutable arguments live separately.
    params: ToolExecutionParams,
    /// Raw arguments retained without read-only value copies.
    args: RefCell<Option<Rc<Value>>>,
    /// Current result retained across callbacks.
    result: RefCell<Option<Rc<ToolExecutionResult>>>,
    /// Independent progress flags.
    phase: Cell<ToolRenderPhase>,
    /// Expansion policy.
    expanded: Cell<bool>,
    /// Image visibility.
    show_images: Cell<bool>,
    /// Constructor retains the supplied width; setters normalize it.
    image_width: Cell<f64>,
    /// Typed state shared across renderer slots.
    state: Rc<RefCell<ToolRendererState>>,
    /// Previous successful call component only.
    call: RefCell<Option<ComponentHandle>>,
    /// Previous successful result component only.
    output: RefCell<Option<ComponentHandle>>,
    /// Weak row invalidator, safe after final owner drop.
    invalidate: Rc<dyn Fn()>,
    /// Current inline children, removable without touching caller-added children.
    images: RefCell<Vec<ComponentHandle>>,
    /// Successful conversions for current payloads.
    converted: RefCell<HashMap<String, HashMap<String, maestro_app::ConvertedImage>>>,
    /// Rebuild failure retained by the infallible invalidation interface.
    error: RefCell<Option<DiagnosticErrorInfo>>,
}
impl ToolExecutionComponent {
    /// Builds the selected shell and its initial call component.
    ///
    /// # Errors
    /// Returns native formatting failures; callback failures use per-slot fallbacks.
    pub fn new(mut params: ToolExecutionParams) -> Result<Self, DiagnosticErrorInfo> {
        let args = params.args.take().map(Rc::new);
        let defined = params.tool_definition.is_some() || params.built_in_tool_definition.is_some();
        let policy = params
            .tool_definition
            .as_ref()
            .and_then(|d| d.render_shell)
            .or_else(|| {
                params
                    .built_in_tool_definition
                    .as_ref()
                    .and_then(|d| d.render_shell)
            });
        let shell = if !defined {
            Shell::Generic(Rc::new(Text::new(String::new(), 1, 1, None)))
        } else if policy == Some(ToolRenderShell::SelfRendered) {
            Shell::SelfRendered(Rc::new(Container::new()))
        } else {
            Shell::Default(Rc::new(Frame::new(1, 1, None)))
        };
        let row = Rc::new_cyclic(|weak: &std::rc::Weak<Row>| {
            let weak = weak.clone();
            let invalidate: Rc<dyn Fn()> = Rc::new(move || Row::request(&weak));
            Row {
                container: Container::new(),
                shell,
                args: RefCell::new(args),
                result: RefCell::default(),
                phase: Cell::new(ToolRenderPhase {
                    execution_started: false,
                    args_complete: false,
                    is_partial: true,
                }),
                expanded: Cell::new(false),
                show_images: Cell::new(params.options.show_images),
                image_width: Cell::new(params.options.image_width_cells),
                state: Rc::default(),
                call: RefCell::default(),
                output: RefCell::default(),
                images: RefCell::default(),
                converted: RefCell::default(),
                error: RefCell::default(),
                invalidate,
                params,
            }
        });
        row.container.add_child(Rc::new(Spacer::new(1)));
        let child: ComponentHandle = match &row.shell {
            Shell::Default(c) => c.clone(),
            Shell::SelfRendered(c) => c.clone(),
            Shell::Generic(c) => c.clone(),
        };
        row.container.add_child(child);
        row.rebuild()?;
        Ok(Self(row))
    }
    /// Replaces raw arguments and rebuilds the retained shell.
    ///
    /// # Errors
    /// Returns a native formatting failure.
    pub fn update_args(&self, args: Option<Value>) -> Result<(), DiagnosticErrorInfo> {
        let old = self.0.args.replace(args.map(Rc::new));
        drop(old);
        self.0.rebuild()
    }
    /// Marks execution started and requests rendering after rebuilding.
    ///
    /// # Errors
    /// Returns a native formatting failure.
    pub fn mark_execution_started(&self) -> Result<(), DiagnosticErrorInfo> {
        self.0.phase.set(ToolRenderPhase {
            execution_started: true,
            ..self.0.phase.get()
        });
        self.0.rebuild()?;
        (self.0.params.request_render)();
        Ok(())
    }
    /// Marks arguments complete and requests rendering after rebuilding.
    ///
    /// # Errors
    /// Returns a native formatting failure.
    pub fn set_args_complete(&self) -> Result<(), DiagnosticErrorInfo> {
        self.0.phase.set(ToolRenderPhase {
            args_complete: true,
            ..self.0.phase.get()
        });
        self.0.rebuild()?;
        (self.0.params.request_render)();
        Ok(())
    }
    /// Replaces the result, retiring stale image identities before synchronous conversion.
    ///
    /// # Errors
    /// Returns a native formatting failure.
    pub fn update_result(
        &self,
        result: ToolExecutionResult,
        is_partial: bool,
    ) -> Result<(), DiagnosticErrorInfo> {
        let result = Rc::new(result);
        let old = self.0.result.replace(Some(result.clone()));
        drop(old);
        self.0.phase.set(ToolRenderPhase {
            is_partial,
            ..self.0.phase.get()
        });
        let identities: HashSet<_> = result
            .content
            .iter()
            .filter_map(|block| match block {
                UserBlock::Image(image) => Some((image.data.as_str(), image.mime_type.as_str())),
                UserBlock::Text(_) => None,
            })
            .collect();
        self.0.converted.borrow_mut().retain(|data, formats| {
            formats.retain(|mime, _| identities.contains(&(data.as_str(), mime.as_str())));
            !formats.is_empty()
        });
        self.0.rebuild()?;
        self.0.convert_images(&result)
    }
    /// Changes result expansion.
    ///
    /// # Errors
    /// Returns a native formatting failure.
    pub fn set_expanded(&self, expanded: bool) -> Result<(), DiagnosticErrorInfo> {
        self.0.expanded.set(expanded);
        self.0.rebuild()
    }
    /// Changes inline image visibility independently of terminal support.
    ///
    /// # Errors
    /// Returns a native formatting failure.
    pub fn set_show_images(&self, show: bool) -> Result<(), DiagnosticErrorInfo> {
        self.0.show_images.set(show);
        self.0.rebuild()
    }
    /// Floors finite image widths and clamps them to at least one cell.
    ///
    /// # Errors
    /// Returns a native formatting failure.
    pub fn set_image_width_cells(&self, width: f64) -> Result<(), DiagnosticErrorInfo> {
        self.0.image_width.set(width.floor().max(1.0));
        self.0.rebuild()
    }
}
impl Row {
    /// A retained invalidator cannot keep the row or its writer alive.
    fn request(weak: &std::rc::Weak<Self>) {
        if let Some(row) = weak.upgrade() {
            row.invalidate();
            (row.params.request_render)();
        }
    }

    /// Captures each slot's current context after the preceding callback completes.
    fn context<'a>(
        &self,
        args: Option<&'a Value>,
        last: Option<ComponentHandle>,
    ) -> ToolRenderContext<'a, ToolRendererState> {
        ToolRenderContext {
            args,
            tool_call_id: self.params.tool_call_id.clone(),
            invalidate: self.invalidate.clone(),
            environment: self.params.environment.clone(),
            last_component: last,
            state: self.state.clone(),
            renderer_state: None,
            cwd: self.params.cwd.clone(),
            phase: self.phase.get(),
            expanded: self.expanded.get(),
            show_images: self.show_images.get(),
            is_error: self.result.borrow().as_ref().is_some_and(|r| r.is_error),
        }
    }
    /// Paints the fallback title using the current theme.
    fn title(&self, theme: &Theme) -> Result<String, DiagnosticErrorInfo> {
        theme
            .fg(
                &ThemeColor::ToolTitle,
                &Theme::bold(&self.params.tool_name, true),
            )
            .map_err(|e| DiagnosticErrorInfo::error(e.to_string()))
    }
    /// Sanitized text and capability-aware indicators remain owned by the tool layer.
    fn text(&self) -> Result<String, DiagnosticErrorInfo> {
        let result = self.result.borrow().clone();
        get_text_output(
            result.as_ref().map(|r| r.borrowed()).as_ref(),
            self.show_images.get(),
            &self.params.environment.terminal_image,
        )
    }
    /// Selects each callback independently and keeps only successful previous handles.
    fn components(&self, append: impl Fn(ComponentHandle)) -> Result<(), DiagnosticErrorInfo> {
        let custom = self.params.tool_definition.as_ref();
        let native = self.params.built_in_tool_definition.as_ref();
        let call = custom
            .and_then(|d| d.render_call.as_ref())
            .or_else(|| native.and_then(|d| d.render_call.as_ref()));
        let args = self.args.borrow().clone();
        let last = self.call.borrow().clone();
        let rendered = call.and_then(|f| {
            f(
                args.as_deref(),
                &self.params.environment.live_theme,
                &self.context(args.as_deref(), last),
            )
            .ok()
        });
        let old = self.call.replace(rendered.clone());
        drop(old);
        append(match rendered {
            Some(c) => c,
            None => Rc::new(Text::new(self.title(self.theme()?.as_ref())?, 0, 0, None)),
        });
        let result = self.result.borrow().clone();
        if let Some(result) = result {
            let callback = custom
                .and_then(|d| d.render_result.as_ref())
                .or_else(|| native.and_then(|d| d.render_result.as_ref()));
            let args = self.args.borrow().clone();
            let last = self.output.borrow().clone();
            let context = self.context(args.as_deref(), last);
            let options = ToolRenderResultOptions {
                expanded: context.expanded,
                is_partial: context.phase.is_partial,
            };
            let rendered = callback.and_then(|f| {
                f.prepare(result.borrowed())
                    .ok()?
                    .render(options, &self.params.environment.live_theme, &context)
                    .ok()
            });
            let old = self.output.replace(rendered.clone());
            drop(old);
            let child = match rendered {
                Some(c) => Some(c),
                None => self.result_fallback()?,
            };
            if let Some(child) = child {
                append(child);
            }
        }
        Ok(())
    }
    /// Builds the text fallback only when a result callback is absent or failed: every line
    /// when expanded, otherwise the first lines and an expand hint counting the rest.
    fn result_fallback(&self) -> Result<Option<ComponentHandle>, DiagnosticErrorInfo> {
        let text = self.text()?;
        if text.is_empty() {
            return Ok(None);
        }
        let theme = self.theme()?;
        let lines: Vec<&str> = text.split('\n').collect();
        let shown = if self.expanded.get() {
            lines.len()
        } else {
            lines.len().min(FALLBACK_PREVIEW_LINES)
        };
        let mut text = lines[..shown]
            .iter()
            .map(|line| theme.fg(&ThemeColor::ToolOutput, line))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DiagnosticErrorInfo::error(e.to_string()))?
            .join("\n");
        let remaining = lines.len() - shown;
        if remaining > 0 {
            text += &expand_hint(
                &format!("{remaining} more lines"),
                &theme,
                &self.params.environment.live_theme,
            )?;
        }
        Ok(Some(Rc::new(Text::new(text, 0, 0, None))))
    }
    /// Samples the live theme at the consuming callback or fallback boundary.
    fn theme(&self) -> Result<Rc<Theme>, DiagnosticErrorInfo> {
        self.params
            .environment
            .live_theme
            .get()
            .map_err(|e| DiagnosticErrorInfo::error(e.to_string()))
    }
    /// Rebuilds only the retained shell and row-owned image children.
    fn rebuild(&self) -> Result<(), DiagnosticErrorInfo> {
        let theme = self.theme()?;
        let bg = if self.phase.get().is_partial {
            ThemeBg::ToolPendingBg
        } else if self.result.borrow().as_ref().is_some_and(|r| r.is_error) {
            ThemeBg::ToolErrorBg
        } else {
            ThemeBg::ToolSuccessBg
        };
        let background = super::background(self.params.environment.live_theme.clone(), bg);
        match &self.shell {
            Shell::Generic(text) => {
                text.set_custom_bg_fn(Some(background));
                let mut value = self.title(&theme)?;
                let args = self.args.borrow().clone();
                if let Some(args) = args {
                    value.push_str("\n\n");
                    value.push_str(
                        &pretty_json(&args)
                            .map_err(|e| DiagnosticErrorInfo::error(e.to_string()))?,
                    );
                }
                let output = self.text()?;
                if !output.is_empty() {
                    value.push('\n');
                    value.push_str(&output);
                }
                text.set_text(value);
            }
            Shell::Default(frame) => {
                frame.set_bg_fn(Some(background));
                frame.clear();
                self.components(|child| frame.add_child(child))?;
            }
            Shell::SelfRendered(container) => {
                container.clear();
                self.components(|child| container.add_child(child))?;
            }
        }
        self.rebuild_images();
        let old = self.error.take();
        drop(old);
        Ok(())
    }
    /// Renders the children in order. A self-rendered shell whose content draws nothing
    /// drops the blank row before it, so with no images the row takes no space.
    fn rows<E>(
        &self,
        width: usize,
        mut render: impl FnMut(&dyn Component, usize) -> Result<Vec<String>, E>,
    ) -> Result<Vec<String>, E> {
        let Shell::SelfRendered(content) = &self.shell else {
            return render(&self.container, width);
        };
        let children: Vec<ComponentHandle> = self.container.walk().collect();
        let mut rendered = Vec::with_capacity(children.len());
        for child in &children {
            rendered.push(render(child.as_ref(), width)?);
        }
        let at = children
            .iter()
            .position(|child| std::ptr::addr_eq(Rc::as_ptr(child), Rc::as_ptr(content)));
        if let Some(at) = at.filter(|&at| at > 0 && rendered[at].is_empty()) {
            rendered[at - 1].clear();
        }
        Ok(rendered.concat())
    }
    /// Invalidates before rebuilding, retaining failures for fallible rendering.
    fn invalidate(&self) {
        self.container.invalidate();
        if let Err(error) = self.rebuild() {
            let old = self.error.replace(Some(error));
            drop(old);
        }
    }
}
impl std::ops::Deref for ToolExecutionComponent {
    type Target = Container;
    fn deref(&self) -> &Container {
        &self.0.container
    }
}
impl Component for ToolExecutionComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.0
            .rows(width, |child, width| {
                Ok::<_, std::convert::Infallible>(child.render(width))
            })
            .unwrap_or_else(|never| match never {})
    }
    fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        if let Some(error) = self.0.error.borrow().clone() {
            return Err(Box::new(error));
        }
        self.0.rows(width, Component::try_render)
    }
    fn invalidate(&self) {
        self.0.invalidate();
    }
}
