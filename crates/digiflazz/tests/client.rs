use digiflazz::{
    Command, DepositRequest, Digiflazz, Error, PriceList, PriceListKind, Status, TransactionRequest,
};
use serde_json::{json, Value};
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const USERNAME: &str = "user";
const KEY: &str = "key";

fn client(server: &MockServer) -> Digiflazz {
    Digiflazz::with_base_url(USERNAME, KEY, server.uri()).unwrap()
}

#[tokio::test]
async fn cek_saldo_sends_expected_payload() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/cek-saldo"))
        .and(body_partial_json(json!({
            "cmd": "deposit",
            "username": USERNAME,
            "sign": "e9d1db43a49c569bcc2bc7164cbe6127"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "deposit": 500000000000.0 }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let balance = client(&server).cek_saldo().await.unwrap();
    assert_eq!(balance.deposit, 500_000_000_000.0);
}

#[tokio::test]
async fn daftar_harga_prepaid_uses_pricelist_signature() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/price-list"))
        .and(body_partial_json(json!({
            "cmd": "prepaid",
            "username": USERNAME,
            "sign": "6870b202c286b430a1f8a6744d48a862"
        })))
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

    match client(&server)
        .daftar_harga(PriceListKind::Prepaid)
        .await
        .unwrap()
    {
        PriceList::Prepaid(products) => {
            assert_eq!(products.len(), 1);
            assert_eq!(products[0].buyer_sku_code, "X100");
        }
        other => panic!("expected prepaid list, got {other:?}"),
    }
}

#[tokio::test]
async fn daftar_harga_pasca_parses_postpaid_products() {
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

    match client(&server)
        .daftar_harga(PriceListKind::Pasca)
        .await
        .unwrap()
    {
        PriceList::Pasca(products) => {
            assert_eq!(products[0].admin, 2750);
            assert_eq!(products[0].buyer_sku_code, "pln");
        }
        other => panic!("expected postpaid list, got {other:?}"),
    }
}

#[tokio::test]
async fn deposit_maps_owner_name_and_username() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/deposit"))
        .and(body_partial_json(json!({
            "username": USERNAME,
            "amount": 10000000,
            "bank": "BCA",
            "owner_name": "John Doe",
            "sign": "ceec9e72c6645e30e25eae8f8d5c1372"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "rc": "00",
                "bank": "BCA",
                "payment_method": "Bank Transfer",
                "account_no": "0123 4567 89",
                "notes": "A6R5UPV",
                "amount": 10000001
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let ticket = client(&server)
        .deposit(&DepositRequest {
            amount: 10_000_000,
            bank: "BCA".into(),
            owner_name: "John Doe".into(),
        })
        .await
        .unwrap();

    assert_eq!(ticket.rc, "00");
    assert_eq!(ticket.amount, 10_000_001);
    assert_eq!(ticket.notes, "A6R5UPV");
}

#[tokio::test]
async fn prepaid_transaction_signs_with_ref_id() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/transaction"))
        .and(body_partial_json(json!({
            "username": USERNAME,
            "buyer_sku_code": "xld25",
            "customer_no": "087800001233",
            "ref_id": "INV-1",
            "sign": "5a51208d5988ea28f13019dd87cd6bcc",
            "testing": false
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "ref_id": "INV-1",
                "customer_no": "087800001233",
                "buyer_sku_code": "xld25",
                "message": "Transaksi Sukses",
                "status": "Sukses",
                "rc": "00",
                "sn": "S1234554321N",
                "buyer_last_saldo": 100000,
                "price": 25000,
                "tele": "@telegram",
                "wa": "081234512345"
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let transaction = client(&server)
        .transaksi(&TransactionRequest::new("xld25", "087800001233", "INV-1"))
        .await
        .unwrap();

    assert_eq!(transaction.status(), Status::Sukses);
    assert_eq!(transaction.sn, "S1234554321N");
    assert_eq!(transaction.price, Some(25000));
}

#[tokio::test]
async fn postpaid_transaction_sends_commands_and_options() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/transaction"))
        .and(body_partial_json(json!({
            "commands": "inq-pasca",
            "username": USERNAME,
            "buyer_sku_code": "pln",
            "customer_no": "530000000003",
            "ref_id": "INV-2",
            "max_price": 100000,
            "cb_url": "https://example.com/webhook",
            "allow_dot": true
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "ref_id": "INV-2",
                "customer_no": "530000000003",
                "buyer_sku_code": "pln",
                "message": "Transaksi Sukses",
                "status": "Sukses",
                "rc": "00"
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut request =
        TransactionRequest::with_command("pln", "530000000003", "INV-2", Command::Inquiry);
    request.max_price = Some(100_000);
    request.callback_url = Some("https://example.com/webhook".into());
    request.allow_dot = Some(true);

    let transaction = client(&server).transaksi(&request).await.unwrap();
    assert_eq!(transaction.status(), Status::Sukses);
}

#[tokio::test]
async fn prepaid_transaction_omits_postpaid_fields() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/transaction"))
        .respond_with(|request: &wiremock::Request| {
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            for field in ["commands", "max_price", "cb_url", "allow_dot"] {
                assert!(body.get(field).is_none(), "{field} should be omitted");
            }
            ResponseTemplate::new(200).set_body_json(json!({
                "data": { "status": "Pending", "message": "Transaksi Pending" }
            }))
        })
        .mount(&server)
        .await;

    let transaction = client(&server)
        .transaksi(&TransactionRequest::new("xld25", "087800001233", "INV-3"))
        .await
        .unwrap();

    assert_eq!(transaction.status(), Status::Pending);
}

#[tokio::test]
async fn http_error_with_rc_preserves_the_code() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/cek-saldo"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "data": { "rc": "41", "message": "Signature Anda salah" }
        })))
        .mount(&server)
        .await;

    let error = client(&server).cek_saldo().await.unwrap_err();
    match &error {
        Error::Rejected { rc, message } => {
            assert_eq!(rc, "41");
            assert_eq!(message, "Signature Anda salah");
        }
        other => panic!("expected Rejected error, got {other:?}"),
    }
}

#[tokio::test]
async fn api_error_without_rc_keeps_the_status() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/cek-saldo"))
        .respond_with(ResponseTemplate::new(503).set_body_string("Service Unavailable"))
        .mount(&server)
        .await;

    let error = client(&server).cek_saldo().await.unwrap_err();
    match &error {
        Error::Api { status, message } => {
            assert_eq!(*status, 503);
            assert_eq!(message, "Service Unavailable");
        }
        other => panic!("expected Api error, got {other:?}"),
    }

    assert!(error
        .to_string()
        .contains("request failed with status 503: Service Unavailable"));
}

#[tokio::test]
async fn rejects_empty_credentials() {
    assert!(matches!(
        Digiflazz::new("", KEY),
        Err(Error::Config("username must not be empty"))
    ));
    assert!(matches!(
        Digiflazz::new(USERNAME, ""),
        Err(Error::Config("key must not be empty"))
    ));
}

#[tokio::test]
async fn rejection_inside_http_200_is_surfaced() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/price-list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "rc": "83",
                "message": "Anda telah mencapai limitasi pengecekan pricelist, silahkan coba beberapa saat lagi"
            }
        })))
        .mount(&server)
        .await;

    for kind in [PriceListKind::Prepaid, PriceListKind::Pasca] {
        let error = client(&server).daftar_harga(kind).await.unwrap_err();
        match &error {
            Error::Rejected { rc, message } => {
                assert_eq!(rc, "83");
                assert!(message.starts_with("Anda telah mencapai limitasi"));
            }
            other => panic!("expected Rejected error, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn null_price_list_is_an_empty_list() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/price-list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": null })))
        .mount(&server)
        .await;

    match client(&server)
        .daftar_harga(PriceListKind::Pasca)
        .await
        .unwrap()
    {
        PriceList::Pasca(products) => assert!(products.is_empty()),
        other => panic!("expected empty postpaid list, got {other:?}"),
    }
}

#[tokio::test]
async fn success_rc_is_not_treated_as_rejection() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/transaction"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "ref_id": "INV-1",
                "message": "Transaksi Sukses",
                "status": "Sukses",
                "rc": "00"
            }
        })))
        .mount(&server)
        .await;

    let transaction = client(&server)
        .transaksi(&TransactionRequest::new("xld10", "087800001230", "INV-1"))
        .await
        .unwrap();

    assert_eq!(transaction.status(), Status::Sukses);
    assert_eq!(transaction.rc, "00");
}

#[tokio::test]
async fn failed_transaction_is_a_rejection() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/transaction"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "ref_id": "INV-1",
                "customer_no": "087800001232",
                "buyer_sku_code": "xld10",
                "message": "Transaksi Gagal",
                "status": "Gagal",
                "rc": "02"
            }
        })))
        .mount(&server)
        .await;

    let error = client(&server)
        .transaksi(&TransactionRequest::new("xld10", "087800001232", "INV-1"))
        .await
        .unwrap_err();

    match &error {
        Error::Rejected { rc, message } => {
            assert_eq!(rc, "02");
            assert_eq!(message, "Transaksi Gagal");
        }
        other => panic!("expected Rejected error, got {other:?}"),
    }
}
