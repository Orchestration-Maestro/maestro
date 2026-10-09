//! Internal response conversion and event reduction; endpoint adapters supply production callers.

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the standard endpoint provider is its first caller"
    )
)]
pub(crate) mod openai_responses_shared;
