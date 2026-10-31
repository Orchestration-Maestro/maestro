//! Input transforms with explicit image replacement intent.
use super::{ExtensionRunner, dispatch::HandlerCursor};
use crate::{
    EventError, ExtensionExecution,
    types::events::{InputEvent, InputEventResult, InputSource, InputTransform},
};
use maestro_models::ImageContent;
use maestro_session::Presence;
use maestro_session::messages::UserMessageDelivery;

impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Chains input transforms, preserving image edits and stopping on handled input.
    /// # Errors
    /// Returns an error-listener failure.
    pub async fn emit_input(
        &self,
        text: &str,
        images: &mut Presence<Vec<ImageContent>>,
        source: InputSource,
        streaming_behavior: Option<UserMessageDelivery>,
    ) -> Result<InputEventResult, EventError> {
        let delivery = streaming_behavior.map_or(Presence::Missing, Presence::Present);
        let context = self.create_context();
        let mut cursor = HandlerCursor::default();
        let mut current_text = text.to_owned();
        let mut replaced_images = false;
        while let Some(invocation) = cursor.next::<_, InputEventResult>(self, "input") {
            let mut event = InputEvent {
                text: current_text.clone(),
                images: std::mem::take(images),
                source,
                streaming_behavior: delivery.clone(),
            };
            let result = invocation.call(self, &mut event, context.clone()).await;
            *images = event.images;
            let transform = match result? {
                Some(InputEventResult::Handled) => return Ok(InputEventResult::Handled),
                Some(InputEventResult::Transform(transform)) => transform,
                _ => continue,
            };
            current_text = transform.text;
            if let Presence::Present(replacement) = transform.images {
                *images = Presence::Present(replacement);
                replaced_images = true;
            }
        }
        Ok(if current_text != text || replaced_images {
            InputEventResult::Transform(InputTransform {
                text: current_text,
                images: images.clone(),
            })
        } else {
            InputEventResult::Continue
        })
    }
}
