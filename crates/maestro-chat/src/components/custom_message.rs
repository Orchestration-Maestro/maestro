//! Custom messages with an optional retained renderer.
use maestro_app::{
    CustomMessage, ExtensionExecution, MessageRenderOptions, MessageRendererCall, UserBlock,
    UserContent,
};
use maestro_theme::{LiveTheme, ThemeBg, ThemeColor};
use maestro_tui::{
    Box as Panel, Component, Container, MarkdownTheme, Spacer, TerminalImage, Text,
    tui::ComponentHandle,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Extension-supplied message rendering either the callback's component or a default panel.
pub struct CustomMessageComponent<T: 'static, E: ExtensionExecution> {
    /// Editable outer child owner.
    container: Container,
    /// Shared typed message retained for callbacks and fallback.
    message: E::Shared<CustomMessage<T>>,
    /// Selected callback with its invocation owner.
    custom_renderer: Option<MessageRendererCall<T, E>>,
    /// Default padded panel reused across rebuilds.
    panel: Rc<Panel>,
    /// Most recently selected custom child.
    custom_component: RefCell<Option<ComponentHandle>>,
    /// Current expansion flag.
    expanded: Cell<bool>,
    /// Existing live theme and Markdown operands.
    rendering: super::MessageRendering,
}
impl<T: 'static, E: ExtensionExecution> CustomMessageComponent<T, E> {
    /// Retains the message and its optional renderer, then selects one body.
    #[must_use]
    pub fn new(
        message: E::Shared<CustomMessage<T>>,
        custom_renderer: Option<MessageRendererCall<T, E>>,
        theme: LiveTheme,
        markdown_theme: Rc<MarkdownTheme>,
        terminal_image: TerminalImage,
    ) -> Self {
        let container = Container::new();
        container.add_child(Rc::new(Spacer::new(1)));
        let panel = Rc::new(Panel::new(
            1,
            1,
            Some(super::background(theme.clone(), ThemeBg::CustomMessageBg)),
        ));
        let component = Self {
            container,
            message,
            custom_renderer,
            panel,
            custom_component: RefCell::default(),
            expanded: Cell::new(false),
            rendering: super::MessageRendering::new(theme, markdown_theme, terminal_image),
        };
        component.rebuild();
        component
    }
    /// Rebuilds only when the expansion state changes.
    pub fn set_expanded(&self, expanded: bool) {
        if self.expanded.replace(expanded) != expanded {
            self.rebuild();
        }
    }
    /// Replaces only the body this component owns, keeping unrelated outer edits.
    fn rebuild(&self) {
        if let Some(previous) = self.custom_component.take() {
            self.container.remove_child(&previous);
        }
        let panel: ComponentHandle = self.panel.clone();
        self.container.remove_child(&panel);
        let selected = self.custom_renderer.as_ref().and_then(|renderer| {
            renderer
                .call(
                    self.message.clone(),
                    MessageRenderOptions {
                        expanded: self.expanded.get(),
                    },
                    self.rendering.theme.clone(),
                )
                .ok()
                .flatten()
        });
        if let Some(child) = selected {
            self.container.add_child(child.clone());
            self.custom_component.replace(Some(child));
            return;
        }
        self.default_body(panel);
    }
    /// Composes the label, spacer and Markdown body inside the panel.
    fn default_body(&self, panel: ComponentHandle) {
        self.container.add_child(panel);
        self.panel.clear();
        let label = self.rendering.paint(
            &ThemeColor::CustomMessageLabel,
            &format!("\x1b[1m[{}]\x1b[22m", self.message.custom_type),
        );
        self.panel.add_child(Rc::new(Text::new(label, 0, 0, None)));
        self.panel.add_child(Rc::new(Spacer::new(1)));
        let text = match &self.message.content {
            UserContent::Text(text) => text.clone(),
            UserContent::Blocks(blocks) => UserBlock::text_output(blocks),
        };
        self.panel.add_child(self.rendering.markdown(
            text,
            0,
            Some(ThemeColor::CustomMessageText),
            false,
        ));
    }
}
impl<T: 'static, E: ExtensionExecution> std::ops::Deref for CustomMessageComponent<T, E> {
    type Target = Container;
    fn deref(&self) -> &Self::Target {
        &self.container
    }
}
impl<T: 'static, E: ExtensionExecution> Component for CustomMessageComponent<T, E> {
    fn render(&self, width: usize) -> Vec<String> {
        self.container.render(width)
    }
    fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        self.container.try_render(width)
    }
    fn invalidate(&self) {
        self.container.invalidate();
        self.rebuild();
    }
}
