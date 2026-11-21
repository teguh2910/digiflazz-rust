//! CORS behaviour for browser clients.

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use digiflazz_service::state::AppState;
use digiflazz_service::{routes, Config};
use tower::ServiceExt;

const ALLOWED: &str = "https://app.example.com";
const TOKEN: &str = "test-token-at-least-16";

fn app_with(origins: &[&str]) -> axum::Router {
    let config = Config {
        username: "user".to_owned(),
        api_key: "key".to_owned(),
        base_url: "http://127.0.0.1:1".to_owned(),
        production: false,
        api_token: TOKEN.to_owned(),
        port: 0,
        price_list_ttl: Duration::from_secs(300),
        cors_origins: origins.iter().map(|origin| (*origin).to_owned()).collect(),
        mock_mode: false,
    };

    routes::router(AppState::new(config).unwrap())
}

fn preflight(origin: &str) -> Request<Body> {
    Request::builder()
        .method("OPTIONS")
        .uri("/transaction")
        .header("origin", origin)
        .header("access-control-request-method", "POST")
        .header(
            "access-control-request-headers",
            "authorization,content-type",
        )
        .body(Body::empty())
        .unwrap()
}

fn get_with_origin(uri: &str, origin: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("origin", origin)
        .header("authorization", format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn preflight_from_allowed_origin_succeeds() {
    let response = app_with(&[ALLOWED])
        .oneshot(preflight(ALLOWED))
        .await
        .unwrap();

    assert!(response.status().is_success());
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some(ALLOWED)
    );
}

#[tokio::test]
async fn preflight_from_foreign_origin_is_not_allowed() {
    let response = app_with(&[ALLOWED])
        .oneshot(preflight("https://evil.example"))
        .await
        .unwrap();

    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none(),
        "foreign origin must not receive an allow-origin header"
    );
}

#[tokio::test]
async fn no_origins_configured_rejects_preflight() {
    let response = app_with(&[]).oneshot(preflight(ALLOWED)).await.unwrap();

    assert!(response
        .headers()
        .get("access-control-allow-origin")
        .is_none());
}

#[tokio::test]
async fn allowed_origin_response_exposes_request_id() {
    let response = app_with(&[ALLOWED])
        .oneshot(get_with_origin("/health", ALLOWED))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some(ALLOWED)
    );

    let exposed = response
        .headers()
        .get("access-control-expose-headers")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();

    assert!(
        exposed.to_ascii_lowercase().contains("x-request-id"),
        "browser needs x-request-id exposed, got {exposed:?}"
    );
}

#[tokio::test]
async fn preflight_allows_authorization_header() {
    let response = app_with(&[ALLOWED])
        .oneshot(preflight(ALLOWED))
        .await
        .unwrap();

    let allowed = response
        .headers()
        .get("access-control-allow-headers")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();

    assert!(
        allowed.contains("authorization"),
        "browser must be allowed to send Authorization, got {allowed:?}"
    );
}
