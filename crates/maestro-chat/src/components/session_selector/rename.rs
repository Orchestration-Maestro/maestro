//! Stable rename editor and shared base-layout replacement.
use super::{
    Component, Container, DynamicBorder, Focusable, LoadReason, Rc, SessionList, Spacer, State,
    ThemeColor, paint,
};
use maestro_app::{is_whitespace_scalar, key_text};
use maestro_theme::Theme;
use maestro_tui::Text;
impl State {
    /// Keeps list callbacks useful without a strong backedge from the editor.
    pub(super) fn wire_rename(self: &Rc<Self>, list: &SessionList) {
        let state = self.clone();
        list.set_on_rename_session(Some(Rc::new(move |path| state.enter_rename(path))));
        let weak = Rc::downgrade(self);
        self.rename_input.set_on_submit(Some(Rc::new(move |value| {
            if let Some(state) = weak.upgrade() {
                state.submit_rename(value);
            }
        })));
    }
    /// Prefills without resetting the retained edit cursor.
    fn enter_rename(&self, path: &str) {
        let discovery = self.discovery(self.scope.get());
        if self.rename.is_none() || discovery.loading.get() {
            return;
        }
        let name = discovery
            .records
            .borrow()
            .as_ref()
            .and_then(|records| records.iter().find(|record| record.path == path))
            .and_then(|record| record.name.clone())
            .unwrap_or_default();
        self.rename_target.replace(Some(path.to_owned()));
        self.rename_input.set_value(name);
        self.rename_input.focus_flag().set(true);
        let panel = Rc::new(Container::new());
        panel.add_child(Rc::new(Text::new(
            Theme::bold("Rename Session", (self.styles)()),
            1,
            0,
            None,
        )));
        panel.add_child(Rc::new(Spacer::new(1)));
        panel.add_child(self.rename_input.clone());
        panel.add_child(Rc::new(Spacer::new(1)));
        let help = format!(
            "{} to save · {} to cancel",
            key_text("tui.select.confirm"),
            key_text("tui.select.cancel")
        );
        panel.add_child(Rc::new(Text::new(
            paint(self.theme.get().ok().as_deref(), &ThemeColor::Muted, &help),
            1,
            0,
            None,
        )));
        self.build_layout(panel, false);
        (self.render)();
    }
    /// Restores the original list and invalidates stale editor submissions.
    pub(super) fn exit_rename(&self) {
        self.rename_target.replace(None);
        if let Some(list) = self.list.upgrade() {
            self.build_layout(list, true);
        }
        (self.render)();
    }
    /// Admits each nonblank save independently before scheduling its completion.
    fn submit_rename(self: &Rc<Self>, value: &str) {
        let name = value.trim_matches(is_whitespace_scalar);
        if name.is_empty() {
            return;
        }
        let target = self.rename_target.borrow().clone();
        let (Some(target), Some(callback)) = (target, self.rename.as_ref()) else {
            self.exit_rename();
            return;
        };
        let save = callback(&target, name);
        let state = self.clone();
        self.runtime.spawn_local(Box::pin(async move {
            let result = async {
                save.await?;
                state.load(state.scope.get(), LoadReason::Refresh).await
            }
            .await;
            state.exit_rename();
            result
        }));
    }
    /// Replaces child-array identity while reusing live border and header owners.
    pub(super) fn build_layout(&self, content: Rc<dyn Component>, show_header: bool) {
        let Some(container) = self.container.upgrade() else {
            return;
        };
        container.clear();
        container.add_child(Rc::new(Spacer::new(1)));
        let theme = self.theme.clone();
        let border = Rc::new(move |text: &str| {
            paint(theme.get().ok().as_deref(), &ThemeColor::Accent, text)
        });
        container.add_child(Rc::new(DynamicBorder::new(
            self.theme.clone(),
            Some(border.clone()),
        )));
        container.add_child(Rc::new(Spacer::new(1)));
        if show_header {
            container.add_child(self.header.clone());
            container.add_child(Rc::new(Spacer::new(1)));
        }
        container.add_child(content);
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(Rc::new(DynamicBorder::new(
            self.theme.clone(),
            Some(border),
        )));
    }
}
