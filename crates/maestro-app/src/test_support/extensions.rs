//! Inline extensions that register one command or one input handler.
use maestro_extensions::types::{CommandHandler, RegisteredCommandOptions};
use maestro_extensions::{ExtensionFactory, InputEvent, InputEventResult};
use maestro_models::Local;
use std::{cell::RefCell, rc::Rc};

/// The extension registering the command `name`, whose handler records its arguments in
/// `heard` and then fails with `failure` when one is given.
#[must_use]
pub fn command_extension(
    name: &'static str,
    heard: Rc<RefCell<Option<String>>>,
    failure: Option<&'static str>,
) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let heard = Rc::clone(&heard);
        let handler: CommandHandler<Local> = Rc::new(move |arguments, _| {
            *heard.borrow_mut() = Some(arguments);
            Box::pin(std::future::ready(
                failure.map_or(Ok(()), |text| Err(text.into())),
            ))
        });
        let options = RegisteredCommandOptions {
            description: Some(name.into()),
            get_argument_completions: None,
            handler,
        };
        Box::pin(std::future::ready(
            api.register_command(name.into(), options),
        ))
    })
}

/// The extension whose input handler answers every input with `answer`.
pub fn input_extension(
    answer: impl Fn(&InputEvent) -> Result<Option<InputEventResult>, &'static str> + 'static,
) -> ExtensionFactory<Local> {
    let answer = Rc::new(answer);
    Rc::new(move |api| {
        let answer = Rc::clone(&answer);
        let registered = api.on::<InputEvent, InputEventResult>(
            "input".into(),
            Rc::new(move |event, _| {
                let result = answer(event).map_err(Into::into);
                Box::pin(std::future::ready(result))
            }),
        );
        Box::pin(std::future::ready(registered.map(drop)))
    })
}
