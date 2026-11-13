//! Integration tests driving the router in-process.

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use digiflazz_service::routes;
use digiflazz_service::state::AppState;
use digiflazz_service::Config;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = "test-token-at-least-16";
const USERNAME: &str = "user";
const KEY: &str = "key";

fn config(base_url: String, production: bool, ttl: Duration) -> Config {
    Config {
        username: USERNAME.to_owned(),
        api_key: KEY.to_owned(),
        base_url,
        production,
        api_token: TOKEN.to_owned(),
        port: 0,
        price_list_ttl: ttl,
    }
}

fn app(base_url: String) -> axum::Router {
    app_with(config(base_url, false, Duration::from_secs(300)))
}

fn app_with(config: Config) -> axum::Router {
    routes::router(AppState::new(config).unwrap())
}

async fn call(app: axum::Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, body)
}

fn authorized(method: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap()
}

fn authorized_json(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {TOKEN}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn health_needs_no_token() {
    let server = MockServer::start().await;
    let (status, body) = call(
        app(server.uri()),
        Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert!(body["version"].is_string());
}

#[tokio::test]
async fn health_does_not_call_upstream() {
    let server = MockServer::start().await;
    // No mocks mounted: any upstream call would fail the test by returning 504.
    let (status, _) = call(
        app(server.uri()),
        Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn protected_route_requires_a_token() {
    let server = MockServer::start().await;
    let (status, body) = call(
        app(server.uri()),
        Request::builder()
            .uri("/balance")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "unauthorized");
}

#[tokio::test]
async fn protected_route_rejects_a_wrong_token() {
    let server = MockServer::start().await;
    let (status, _) = call(
        app(server.uri()),
        Request::builder()
            .uri("/balance")
            .header("authorization", "Bearer wrong-token-wrong-token")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn balance_returns_the_upstream_value() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/cek-saldo"))
        .and(body_partial_json(json!({
            "username": USERNAME,
            "sign": "e9d1db43a49c569bcc2bc7164cbe6127"
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "data": { "deposit": 250000.0 } })),
        )
        .mount(&server)
        .await;

    let (status, body) = call(app(server.uri()), authorized("GET", "/balance")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["deposit"], 250000.0);
}

#[tokio::test]
async fn price_list_defaults_to_prepaid() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/price-list"))
        .and(body_partial_json(json!({ "cmd": "prepaid" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{
                "product_name": "Xl 100.000",
                "category": "Pulsa",
                "brand": "XL",
                "type": "Umum",
                "seller_name": "PT. ABC",
                "price": 98000,
                "buyer_sku_code": "X100",
                "buyer_product_status": true,
                "seller_product_status": true,
                "unlimited_stock": true,
                "stock": 0,
                "multi": true,
                "start_cut_off": "23:45",
                "end_cut_off": "00:15",
                "desc": "Pulsa Xl Rp 100.000"
            }]
        })))
        .mount(&server)
        .await;

    let (status, body) = call(app(server.uri()), authorized("GET", "/price-list")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["kind"], "prepaid");
    assert_eq!(body["products"][0]["buyer_sku_code"], "X100");
}

#[tokio::test]
async fn price_list_accepts_pasca() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/price-list"))
        .and(body_partial_json(json!({ "cmd": "pasca" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{
                "product_name": "Pln Postpaid",
                "category": "Pascabayar",
                "brand": "PLN",
                "seller_name": "PT. ABC",
                "admin": 2750,
                "commission": 1800,
                "buyer_sku_code": "pln",
                "buyer_product_status": true,
                "seller_product_status": true,
                "desc": "-"
            }]
        })))
        .mount(&server)
        .await;

    let (status, body) = call(
        app(server.uri()),
        authorized("GET", "/price-list?kind=pasca"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["kind"], "pasca");
    assert_eq!(body["products"][0]["admin"], 2750);
}

#[tokio::test]
async fn unknown_kind_is_a_bad_request() {
    let server = MockServer::start().await;
    let (status, body) = call(
        app(server.uri()),
        authorized("GET", "/price-list?kind=nope"),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "bad_request");
}

#[tokio::test]
async fn null_price_list_becomes_an_empty_array() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/price-list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": null })))
        .mount(&server)
        .await;

    let (status, body) = call(
        app(server.uri()),
        authorized("GET", "/price-list?kind=pasca"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["products"], json!([]));
}

#[tokio::test]
async fn price_list_is_cached() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/price-list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": [] })))
        .expect(1)
        .mount(&server)
        .await;

    let app = app(server.uri());
    call(app.clone(), authorized("GET", "/price-list")).await;
    let (status, _) = call(app, authorized("GET", "/price-list")).await;

    assert_eq!(status, StatusCode::OK);
    // `expect(1)` is asserted when the server is dropped.
}

#[tokio::test]
async fn price_list_is_refetched_after_ttl() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/price-list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": [] })))
        .expect(2)
        .mount(&server)
        .await;

    let app = app_with(config(server.uri(), false, Duration::from_secs(0)));
    call(app.clone(), authorized("GET", "/price-list")).await;
    let (status, _) = call(app, authorized("GET", "/price-list")).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn upstream_rejection_becomes_502_with_rc() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/price-list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "rc": "83", "message": "limit reached" }
        })))
        .mount(&server)
        .await;

    let (status, body) = call(app(server.uri()), authorized("GET", "/price-list")).await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["error"], "upstream_rejected");
    assert_eq!(body["rc"], "83");
    assert_eq!(body["message"], "limit reached");
}

#[tokio::test]
async fn upstream_http_error_with_rc_becomes_502_rejection() {
    let server = MockServer::start().await;

    // Production answers an invalid SKU with HTTP 400 but still carries `rc`.
    Mock::given(method("POST"))
        .and(path("/cek-saldo"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "data": { "rc": "43", "status": "Gagal", "message": "SKU tidak di temukan atau Non-Aktif" }
        })))
        .mount(&server)
        .await;

    let (status, body) = call(app(server.uri()), authorized("GET", "/balance")).await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["error"], "upstream_rejected");
    assert_eq!(body["rc"], "43");
}

#[tokio::test]
async fn upstream_http_error_without_rc_becomes_502() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/cek-saldo"))
        .respond_with(ResponseTemplate::new(503).set_body_string("Service Unavailable"))
        .mount(&server)
        .await;

    let (status, body) = call(app(server.uri()), authorized("GET", "/balance")).await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["error"], "upstream_error");
    assert_eq!(body["status"], 503);
}

#[tokio::test]
async fn malformed_body_is_a_bad_request() {
    let server = MockServer::start().await;

    let request = Request::builder()
        .method("POST")
        .uri("/transaction")
        .header("authorization", format!("Bearer {TOKEN}"))
        .header("content-type", "application/json")
        .body(Body::from("{not json"))
        .unwrap();

    let (status, body) = call(app(server.uri()), request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "bad_request");
    assert!(body["message"].is_string(), "error body must be JSON");
}

#[tokio::test]
async fn missing_field_is_a_bad_request_naming_the_field() {
    let server = MockServer::start().await;

    let (status, body) = call(
        app(server.uri()),
        authorized_json(
            "POST",
            "/deposit",
            json!({ "amount": 200000, "bank": "Flip" }),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "bad_request");
    assert!(
        body["message"].as_str().unwrap().contains("owner_name"),
        "message should name the missing field, got {}",
        body["message"]
    );
}

#[tokio::test]
async fn transaction_forwards_command_and_options() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/transaction"))
        .and(body_partial_json(json!({
            "username": USERNAME,
            "buyer_sku_code": "pln",
            "customer_no": "530000000001",
            "ref_id": "svc-1",
            "commands": "inq-pasca",
            "testing": true,
            "max_price": 100000
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "ref_id": "svc-1",
                "customer_no": "530000000001",
                "buyer_sku_code": "pln",
                "message": "Transaksi Sukses",
                "status": "Sukses",
                "rc": "00"
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let (status, body) = call(
        app(server.uri()),
        authorized_json(
            "POST",
            "/transaction",
            json!({
                "sku": "pln",
                "customer_no": "530000000001",
                "ref_id": "svc-1",
                "command": "inq-pasca",
                "testing": true,
                "max_price": 100000
            }),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "Sukses");
}

#[tokio::test]
async fn development_defaults_testing_to_true() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/transaction"))
        .and(body_partial_json(json!({ "testing": true })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "ref_id": "svc-1", "status": "Sukses", "message": "ok", "rc": "00" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let (status, _) = call(
        app(server.uri()),
        authorized_json(
            "POST",
            "/transaction",
            json!({ "sku": "xld10", "customer_no": "087800001230", "ref_id": "svc-1" }),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn production_does_not_default_testing_to_true() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/transaction"))
        .respond_with(|request: &wiremock::Request| {
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            assert_eq!(body["testing"], false, "production must not force testing");
            ResponseTemplate::new(200).set_body_json(json!({
                "data": { "ref_id": "svc-1", "status": "Sukses", "message": "ok", "rc": "00" }
            }))
        })
        .expect(1)
        .mount(&server)
        .await;

    let app = app_with(config(server.uri(), true, Duration::from_secs(300)));
    let (status, _) = call(
        app,
        authorized_json(
            "POST",
            "/transaction",
            json!({ "sku": "xld10", "customer_no": "087800001230", "ref_id": "svc-1" }),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn unknown_command_is_a_bad_request() {
    let server = MockServer::start().await;

    let (status, body) = call(
        app(server.uri()),
        authorized_json(
            "POST",
            "/transaction",
            json!({ "sku": "pln", "customer_no": "1", "ref_id": "r", "command": "nope" }),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "bad_request");
}

#[tokio::test]
async fn deposit_returns_the_ticket() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/deposit"))
        .and(body_partial_json(json!({
            "username": USERNAME,
            "amount": 200000,
            "bank": "Flip",
            "owner_name": "John Doe"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "rc": "00",
                "bank": "Flip (BCA)",
                "payment_method": "Bank Transfer",
                "account_no": "6042888890",
                "notes": "",
                "amount": 201927
            }
        })))
        .mount(&server)
        .await;

    let (status, body) = call(
        app(server.uri()),
        authorized_json(
            "POST",
            "/deposit",
            json!({ "amount": 200000, "bank": "Flip", "owner_name": "John Doe" }),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["rc"], "00");
    assert_eq!(body["amount"], 201927);
}
