//! Weak clipboard completion with an owned replaceable restoring timer.
use super::{attachment_content::AttachmentHost, timer::Timer};
use std::{cell::RefCell, rc::Rc};

/// Feedback owned by one retained control, never by pending clipboard work.
#[derive(Default)]
pub(crate) struct CopyFeedback(Rc<RefCell<Option<Timer>>>);
impl CopyFeedback {
    /// Whether a successfully admitted restoring timer is active.
    pub(crate) fn copied(&self) -> bool {
        self.0.borrow().is_some()
    }

    /// Begin native copying synchronously, then schedule only its owned completion.
    pub(crate) fn copy(
        &self,
        host: &Rc<dyn AttachmentHost>,
        input: (&str, super::attachment_content::ClipboardMode),
        delay: i32,
        context: &egui::Context,
    ) {
        let work = host.copy_to_clipboard(input.0, input.1);
        #[cfg(target_arch = "wasm32")]
        let direct = input.1 == super::attachment_content::ClipboardMode::Direct;
        let feedback = Rc::downgrade(&self.0);
        let weak_host = Rc::downgrade(host);
        let context = context.clone();
        host.spawn_local(Box::pin(async move {
            let succeeded = work.await;
            #[cfg(target_arch = "wasm32")]
            if !succeeded && direct {
                crate::utils::clipboard::report_copy_failure();
            }
            if !succeeded {
                return;
            }
            let (Some(feedback), Some(host)) = (feedback.upgrade(), weak_host.upgrade()) else {
                return;
            };
            let timer = host.timer(
                delay,
                false,
                restore(Rc::downgrade(&feedback), context.clone()),
            );
            let old = feedback.replace(timer.ok());
            drop(old);
            context.request_repaint();
        }));
    }
}
/// Restore idle feedback without retaining its UI owner.
fn restore(weak: std::rc::Weak<RefCell<Option<Timer>>>, context: egui::Context) -> Rc<dyn Fn()> {
    Rc::new(move || {
        let Some(feedback) = weak.upgrade() else {
            return;
        };
        let old = feedback.borrow_mut().take();
        drop(old);
        context.request_repaint();
    })
}
