//! Retrieve feed descriptors without registering models or writing catalog files.
mod feeds;
mod input;
mod models_dev;
mod routes;
pub use feeds::{fetch_ai_gateway_models, fetch_open_router_models, load_models_dev_data};
