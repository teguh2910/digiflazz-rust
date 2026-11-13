//! Integration tests for the request id middleware.

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use digiflazz_service::state::AppState;
use digiflazz_service::{routes, Config};
use tower::ServiceExt;

fn app(base_url: String) -> axum::Router {
    app_with_origins(base_url, Vec::new())
}

fn app_with_origins(base_url: String, cors_origins: Vec<String>) -> axum::Router {
    let config = Config {
        username: "user".to_owned(),
        api_key: "key".to_owned(),
        base_url,
        production: false,
        api_token: "test-token-at-least-16".to_owned(),
        port: 0,
        price_list_ttl: Duration::from_secs(300),
        cors_origins,
    };

    routes::router(AppState::new(config).unwrap())
}

async fn request_id_of(uri: &str, supplied: Option<&str>) -> Option<String> {
    let mut builder = Request::builder().uri(uri);

    if let Some(value) = supplied {
        builder = builder.header(routes::REQUEST_ID_HEADER, value);
    }

    let response = app("http://127.0.0.1:1".to_owned())
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();

    response
        .headers()
        .get(routes::REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

#[tokio::test]
async fn generates_a_request_id_when_none_is_sent() {
    let id = request_id_of("/health", None)
        .await
        .expect("missing header");

    // A generated id is a UUID: 36 chars with four dashes.
    assert_eq!(id.len(), 36, "unexpected id {id}");
    assert_eq!(id.matches('-').count(), 4, "unexpected id {id}");
}

#[tokio::test]
async fn echoes_the_caller_supplied_request_id() {
    let id = request_id_of("/health", Some("my-trace-abc")).await;
    assert_eq!(id.as_deref(), Some("my-trace-abc"));
}

#[tokio::test]
async fn request_id_is_present_on_error_responses() {
    let response = app("http://127.0.0.1:1".to_owned())
        .oneshot(
            Request::builder()
                .uri("/balance")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(
        response.headers().contains_key(routes::REQUEST_ID_HEADER),
        "401 responses must still carry a request id"
    );
}
