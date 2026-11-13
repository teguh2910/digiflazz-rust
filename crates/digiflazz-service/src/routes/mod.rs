//! Route handlers.

mod balance;
mod deposit;
mod health;
mod price_list;
mod transaction;

use axum::http::HeaderName;
use axum::routing::{get, post};
use axum::Router;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;

use crate::state::AppState;

/// Header carrying the correlation id, accepted from callers and echoed back.
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Builds the full router.
///
/// `/health` is mounted outside the authenticated group so orchestrators can
/// probe it without credentials, and so it never spends upstream rate limit.
///
/// Every response carries `x-request-id`: the caller's value when supplied,
/// otherwise a generated UUID.
pub fn router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/balance", get(balance::balance))
        .route("/price-list", get(price_list::price_list))
        .route("/transaction", post(transaction::transaction))
        .route("/deposit", post(deposit::deposit));

    let header = HeaderName::from_static(REQUEST_ID_HEADER);

    Router::new()
        .route("/health", get(health::health))
        .merge(protected)
        .layer(PropagateRequestIdLayer::new(header.clone()))
        .layer(TraceLayer::new_for_http())
        .layer(SetRequestIdLayer::new(header, MakeRequestUuid))
        .with_state(state)
}
