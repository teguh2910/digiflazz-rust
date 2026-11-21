//! HTTP microservice exposing the Digiflazz buyer API.

#![deny(missing_docs)]

pub mod auth;
pub mod cache;
pub mod config;
pub mod error;
pub mod json;
pub mod mock;
pub mod routes;
pub mod state;

pub use config::Config;
pub use error::ApiError;
pub use state::AppState;
