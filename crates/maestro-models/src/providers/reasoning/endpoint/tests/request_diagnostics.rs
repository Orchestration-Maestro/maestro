//! Authored context and scalar-safe diagnostic bodies.

#[test]
fn request_error_body_clips_on_scalar_boundaries() {
    super::request_corpus::corpus("request_error_body_clips_on_scalar_boundaries", 11);
}

#[test]
fn request_error_messages_keep_authored_context() {
    super::request_corpus::corpus("request_error_messages_keep_authored_context", 4);
}
