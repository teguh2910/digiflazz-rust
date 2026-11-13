//! `GET /health`.

use axum::Json;
use serde_json::{json, Value};

/// Liveness probe. No auth, no upstream call.
pub async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
