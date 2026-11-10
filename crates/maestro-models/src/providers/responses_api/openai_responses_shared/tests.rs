//! Controlled witnesses for internal response conversion and reduction.

mod events;
mod requests;
mod responses;

/// Fallible test body result.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
