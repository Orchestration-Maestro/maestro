//! First-message assembly observed through the public operation.

use crate::support;
use maestro_app::ImageContent;
use maestro_cli::input::args::parse_args;
use maestro_cli::input::initial_message::{InitialMessageInput, build_initial_message};

/// One recorded attachment used by the image cases.
fn image() -> ImageContent {
    ImageContent {
        data: "aW1hZ2U=".to_owned(),
        mime_type: "image/png".to_owned(),
    }
}

/// Build the first message of one case and return what the operation retained.
fn observe(
    messages: &[&str],
    stdin: Option<&str>,
    file_text: Option<&str>,
    images: Vec<ImageContent>,
) -> (Option<String>, usize, usize) {
    let argv: Vec<String> = messages
        .iter()
        .map(|message| (*message).to_owned())
        .collect();
    let mut parsed = parse_args(&argv);
    let result = build_initial_message(InitialMessageInput {
        parsed: &mut parsed,
        file_text,
        file_images: images,
        stdin_content: stdin,
    });
    (
        result.initial_message,
        parsed.messages.len(),
        result.initial_images.map_or(0, |images| images.len()),
    )
}

#[test]
fn stdin_and_first_prompt_form_one_message() {
    let case = support::case_by_title("initial", "initial-127");
    let stdin = case.input["stdinContent"].as_str().unwrap();
    let messages: Vec<&str> = case.input["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    let (message, remaining, images) = observe(&messages, Some(stdin), None, Vec::new());
    assert_eq!(
        message.as_deref(),
        case.expected["initialMessage"].as_str(),
        "the assembled first message is the recorded one"
    );
    assert_eq!(
        remaining,
        case.expected["remaining_messages"]
            .as_array()
            .expect("recorded remaining messages")
            .len(),
        "the prompts after the first stay queued"
    );
    assert_eq!(images, 0, "no attachment is built from stdin and a prompt");
}

#[test]
fn stdin_alone_forms_the_first_message() {
    let mut parsed = parse_args(&[]);
    let result = build_initial_message(InitialMessageInput {
        parsed: &mut parsed,
        file_text: None,
        file_images: Vec::new(),
        stdin_content: Some("only stdin\n"),
    });
    assert_eq!(result.initial_message.as_deref(), Some("only stdin\n"));
    assert!(result.initial_images.is_none());
}

#[test]
fn file_text_precedes_the_first_prompt_and_leaves_later_prompts() {
    let mut parsed = parse_args(&["first".to_owned(), "second".to_owned()]);
    let result = build_initial_message(InitialMessageInput {
        parsed: &mut parsed,
        file_text: Some("<file name=\"a\">\ntext\n</file>\n"),
        file_images: Vec::new(),
        stdin_content: Some("stdin\n"),
    });
    assert_eq!(
        result.initial_message.as_deref(),
        Some("stdin\n<file name=\"a\">\ntext\n</file>\nfirst")
    );
    assert_eq!(parsed.messages.len(), 1);
    assert_eq!(parsed.messages.front().map(String::as_str), Some("second"));
}

#[test]
fn empty_and_image_only_initial_inputs_remain_distinct() {
    let mut parsed = parse_args(&[]);
    let empty = build_initial_message(InitialMessageInput {
        parsed: &mut parsed,
        file_text: Some(""),
        file_images: Vec::new(),
        stdin_content: Some(""),
    });
    assert_eq!(empty.initial_message.as_deref(), Some(""));
    assert!(empty.initial_images.is_none());

    let mut parsed = parse_args(&[]);
    let images = build_initial_message(InitialMessageInput {
        parsed: &mut parsed,
        file_text: None,
        file_images: vec![image()],
        stdin_content: None,
    });
    assert!(images.initial_message.is_none());
    assert_eq!(images.initial_images, Some(vec![image()]));

    let mut parsed = parse_args(&[]);
    let absent = build_initial_message(InitialMessageInput {
        parsed: &mut parsed,
        file_text: None,
        file_images: Vec::new(),
        stdin_content: None,
    });
    assert!(absent.initial_message.is_none());
    assert!(absent.initial_images.is_none());
}
