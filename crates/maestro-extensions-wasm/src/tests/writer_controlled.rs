//! The controlled host's frame writer and custom overlays, recorded in the transcript.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::controlled::{Callback, Controlled, Tui};
use super::renderer_host::{Call, Reply, invoke};
use super::widget_controlled::Widget;
use crate::component_adapter::custom::{CustomOptionsRef, OptionsSourceRef};
use crate::component_adapter::{
    TerminalRequest, WidgetAnswer, WidgetEvent, WriterAnswer, WriterRequest, WriterTarget,
};
use crate::types::ExtensionResult;

/// Describes a focus or overlay target.
fn target(target: &WriterTarget<'_, Controlled>) -> String {
    match target {
        WriterTarget::Widget(widget) => format!("widget {}", widget.0),
        WriterTarget::Component(component) => format!("component {}", component.id),
        WriterTarget::Lent(id) => format!("lent {id}"),
    }
}

/// Describes a terminal request and answers it as a 80 by 24 terminal.
fn terminal(request: &TerminalRequest<'_>) -> (String, WriterAnswer<Controlled>) {
    let done = |line: String| (line, WriterAnswer::Done);
    match request {
        TerminalRequest::Columns => ("columns".into(), WriterAnswer::Count(80)),
        TerminalRequest::Rows => ("rows".into(), WriterAnswer::Count(24)),
        TerminalRequest::KittyProtocolActive => ("kitty".into(), WriterAnswer::Flag(false)),
        TerminalRequest::Write(data) => done(format!("write {data:?}")),
        TerminalRequest::MoveBy(lines) => done(format!("moveBy {lines}")),
        TerminalRequest::SetTitle(title) => done(format!("setTitle {title:?}")),
        TerminalRequest::SetProgress(active) => done(format!("setProgress {active}")),
        TerminalRequest::Stop => done("stop".into()),
        TerminalRequest::HideCursor => done("hideCursor".into()),
        TerminalRequest::ShowCursor => done("showCursor".into()),
        TerminalRequest::ClearLine => done("clearLine".into()),
        TerminalRequest::ClearFromCursor => done("clearFromCursor".into()),
        TerminalRequest::ClearScreen => done("clearScreen".into()),
    }
}

impl Controlled {
    /// Records one frame writer request and answers it.
    pub(super) fn writer_request(
        &self,
        widget: &Widget,
        request: &WriterRequest<'_, Self>,
    ) -> WriterAnswer<Self> {
        let done = |line: String| (line, WriterAnswer::Done);
        let (line, answer) = match request {
            WriterRequest::GetShowHardwareCursor => {
                ("getShowHardwareCursor".into(), WriterAnswer::Flag(true))
            }
            WriterRequest::GetClearOnShrink => {
                ("getClearOnShrink".into(), WriterAnswer::Flag(true))
            }
            WriterRequest::HasOverlay => ("hasOverlay".into(), WriterAnswer::Flag(true)),
            WriterRequest::LentLive(id) => (format!("lentLive {id}"), WriterAnswer::Flag(true)),
            WriterRequest::ShowOverlay(component, options) => (
                format!("showOverlay {} {}", target(component), options.is_some()),
                WriterAnswer::Overlay(Tui::lent(&self.observed)),
            ),
            WriterRequest::SetShowHardwareCursor(enabled) => {
                done(format!("setShowHardwareCursor {enabled}"))
            }
            WriterRequest::SetClearOnShrink(enabled) => done(format!("setClearOnShrink {enabled}")),
            WriterRequest::SetFocus(focus) => {
                done(format!("setFocus {:?}", focus.as_ref().map(target)))
            }
            WriterRequest::SetOnDebug(callback) => {
                done(format!("setOnDebug {}", callback.is_some()))
            }
            WriterRequest::RequestRender(force) => done(format!("requestRender {force}")),
            WriterRequest::HideOverlay => done("hideOverlay".into()),
            WriterRequest::Start => done("start".into()),
            WriterRequest::Stop => done("stop".into()),
            WriterRequest::TerminalStart(input, resize) => {
                done(format!("terminal start {} {}", input.id, resize.id))
            }
            WriterRequest::Terminal(request) => {
                let (line, answer) = terminal(request);
                (format!("terminal {line}"), answer)
            }
        };
        let _ = widget;
        self.observe().log(format!("writer {line}"));
        answer
    }

    /// Builds a custom overlay, resolves its placement, hands over its handle, then checks its
    /// visibility around a failing check and completes it.
    pub(super) async fn custom_overlay(
        &self,
        factory: &Callback,
        options: CustomOptionsRef<'_, Self>,
    ) -> ExtensionResult<()> {
        let emitter = super::bus_host::Emitter { key: 0 };
        let (built, close) = self.custom_build(factory).await?;
        let resolved;
        let placement = match options.overlay_options {
            Some(OptionsSourceRef::Placement(options)) => options,
            Some(OptionsSourceRef::Resolver(resolve)) => {
                resolved = self.resolve(resolve, &emitter)?;
                &resolved
            }
            None => return Err("an overlay needs placement in the controlled host".into()),
        };
        let placed = placement.value.get()?;
        self.observe().log(format!("custom overlay {placed}"));
        if let Some(receive) = options.on_handle {
            let handle = WidgetEvent::OverlayHandle(Tui::lent(&self.observed));
            crate::component_adapter::invoke_widget(self, receive.id, handle, &emitter)?;
        }
        self.visible(placement, &emitter);
        let render = |built: &crate::ExtensionComponent| match invoke(
            &built.component,
            Call::Render(20),
            self,
            0,
        ) {
            Ok(Reply::Lines(rows)) => rows.join("|"),
            other => format!("{:?}", other.err()),
        };
        let rows = render(&built);
        self.observe().log(format!("custom overlay rows {rows}"));
        invoke(&built.component, Call::Input("fail"), self, 0)?;
        self.visible(placement, &emitter);
        invoke(&built.component, Call::Input("commit"), self, 0)?;
        assert!(close.is_closed());
        Ok(())
    }

    /// Runs the placement resolver and takes another owner of the record it answers.
    fn resolve(
        &self,
        resolve: &Callback,
        emitter: &super::bus_host::Emitter,
    ) -> ExtensionResult<super::controlled::OverlayOptions> {
        let answer = crate::component_adapter::invoke_widget(
            self,
            resolve.id,
            WidgetEvent::OverlayOptions,
            emitter,
        )?;
        match answer {
            WidgetAnswer::OverlayOptions(options) => {
                crate::component_adapter::overlays::share(self, &options)
            }
            _ => Err("expected overlay options".into()),
        }
    }

    /// Runs the record's visibility check, if any, at 80 by 24 and records its answer.
    fn visible(
        &self,
        placement: &super::controlled::OverlayOptions,
        emitter: &super::bus_host::Emitter,
    ) {
        let Some(id) = placement.visible.get() else {
            return;
        };
        let answer = crate::component_adapter::invoke_widget(
            self,
            id,
            WidgetEvent::Visible(80, 24),
            emitter,
        );
        let line = match answer {
            Ok(WidgetAnswer::Visible(visible)) => format!("visible {visible}"),
            Ok(_) => "visible answer kind mismatch".to_owned(),
            Err(error) => format!("visible failed: {error}"),
        };
        self.observe().log(line);
    }
}
