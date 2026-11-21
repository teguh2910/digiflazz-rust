//! Route handlers.

mod balance;
mod deposit;
mod health;
mod price_list;
mod transaction;

use axum::http::{header, HeaderName, HeaderValue, Method};
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::{AllowOrigin, CorsLayer};
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
    let cors = build_cors(state.config().cors_origins.as_slice());

    let protected = Router::new()
        .route("/balance", get(balance::balance))
        .route("/price-list", get(price_list::price_list))
        .route("/transaction", post(transaction::transaction))
        .route("/deposit", post(deposit::deposit));

    let header = HeaderName::from_static(REQUEST_ID_HEADER);

    Router::new()
        .route("/health", get(health::health))
        .route("/api/v1/health", get(health::health))
        .merge(protected)
        .layer(PropagateRequestIdLayer::new(header.clone()))
        .layer(
            TraceLayer::new_for_http()
                .on_failure(tower_http::trace::DefaultOnFailure::new().level(tracing::Level::WARN)),
        )
        .layer(SetRequestIdLayer::new(header, MakeRequestUuid))
        .layer(cors)
        .with_state(state)
}

/// Builds a CORS layer restricted to the configured origins.
///
/// An empty list yields a layer that answers no cross-origin request, which
/// keeps the previous server-to-server behaviour by default. Origins are matched
/// exactly; wildcards are deliberately not supported so a typo cannot silently
/// allow every site to read responses.
fn build_cors(origins: &[String]) -> CorsLayer {
    let parsed: Vec<HeaderValue> = origins
        .iter()
        .filter_map(|origin| match origin.parse() {
            Ok(value) => Some(value),
            Err(_) => {
                tracing::warn!(origin = %origin, "ignoring invalid CORS origin");
                None
            }
        })
        .collect();

    let layer = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT])
        .expose_headers([HeaderName::from_static(REQUEST_ID_HEADER)])
        .max_age(std::time::Duration::from_secs(600));

    if parsed.is_empty() {
        // Nothing configured: reject every cross-origin request.
        layer.allow_origin(AllowOrigin::list(Vec::new()))
    } else {
        tracing::info!(origins = ?origins, "CORS enabled");
        layer.allow_origin(AllowOrigin::list(parsed))
    }
}

#[cfg(test)]
mod tests {
    use super::build_cors;

    #[test]
    fn empty_origin_list_is_accepted_without_panicking() {
        let _ = build_cors(&[]);
    }

    #[test]
    fn invalid_origin_is_skipped() {
        let _ = build_cors(&[
            "not a valid origin".to_owned(),
            "https://ok.example".to_owned(),
        ]);
    }
}
