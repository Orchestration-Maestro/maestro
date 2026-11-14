//! Shared integration-test executable.
#![cfg(test)]

use maestro_http::test_support::runtime;
// The committed boundary fixture names the descriptor record as `crate::Model`.
use maestro_interchange::types::Model;

mod catalog_command;
#[path = "support/catalog_command.rs"]
mod catalog_command_support;
mod catalog_generation;
mod circle_fixture;
