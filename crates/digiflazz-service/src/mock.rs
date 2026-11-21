//! Mock data provider for offline/sandbox testing without calling Digiflazz upstream.

use digiflazz::{
    Balance, DepositRequest, DepositTicket, PostpaidProduct, PrepaidProduct, PriceList,
    PriceListKind, Transaction, TransactionRequest,
};
use serde_json::json;

/// Returns a simulated balance.
pub fn get_mock_balance() -> Balance {
    Balance {
        deposit: 5_000_000.0,
    }
}

/// Returns a simulated price list.
pub fn get_mock_price_list(kind: PriceListKind) -> PriceList {
    match kind {
        PriceListKind::Prepaid => {
            let products = vec![
                PrepaidProduct {
                    product_name: "Telkomsel 5.000".into(),
                    category: "Pulsa".into(),
                    brand: "TELKOMSEL".into(),
                    kind: "Umum".into(),
                    seller_name: "DigiMock".into(),
                    price: 5800,
                    buyer_sku_code: "htelkomsel5".into(),
                    buyer_product_status: true,
                    seller_product_status: true,
                    unlimited_stock: true,
                    stock: 999,
                    multi: true,
                    start_cut_off: "23:50".into(),
                    end_cut_off: "00:10".into(),
                    desc: "Pulsa Telkomsel Rp 5.000".into(),
                },
                PrepaidProduct {
                    product_name: "Telkomsel 10.000".into(),
                    category: "Pulsa".into(),
                    brand: "TELKOMSEL".into(),
                    kind: "Umum".into(),
                    seller_name: "DigiMock".into(),
                    price: 10450,
                    buyer_sku_code: "htelkomsel10".into(),
                    buyer_product_status: true,
                    seller_product_status: true,
                    unlimited_stock: true,
                    stock: 999,
                    multi: true,
                    start_cut_off: "23:50".into(),
                    end_cut_off: "00:10".into(),
                    desc: "Pulsa Telkomsel Rp 10.000".into(),
                },
                PrepaidProduct {
                    product_name: "Telkomsel 20.000".into(),
                    category: "Pulsa".into(),
                    brand: "TELKOMSEL".into(),
                    kind: "Umum".into(),
                    seller_name: "DigiMock".into(),
                    price: 20200,
                    buyer_sku_code: "htelkomsel20".into(),
                    buyer_product_status: true,
                    seller_product_status: true,
                    unlimited_stock: true,
                    stock: 999,
                    multi: true,
                    start_cut_off: "23:50".into(),
                    end_cut_off: "00:10".into(),
                    desc: "Pulsa Telkomsel Rp 20.000".into(),
                },
                PrepaidProduct {
                    product_name: "Indosat 10.000".into(),
                    category: "Pulsa".into(),
                    brand: "INDOSAT".into(),
                    kind: "Umum".into(),
                    seller_name: "DigiMock".into(),
                    price: 10300,
                    buyer_sku_code: "hindosat10".into(),
                    buyer_product_status: true,
                    seller_product_status: true,
                    unlimited_stock: true,
                    stock: 999,
                    multi: true,
                    start_cut_off: "23:50".into(),
                    end_cut_off: "00:10".into(),
                    desc: "Pulsa Indosat Rp 10.000".into(),
                },
                PrepaidProduct {
                    product_name: "Indosat 25.000".into(),
                    category: "Pulsa".into(),
                    brand: "INDOSAT".into(),
                    kind: "Umum".into(),
                    seller_name: "DigiMock".into(),
                    price: 24900,
                    buyer_sku_code: "hindosat25".into(),
                    buyer_product_status: true,
                    seller_product_status: true,
                    unlimited_stock: true,
                    stock: 999,
                    multi: true,
                    start_cut_off: "23:50".into(),
                    end_cut_off: "00:10".into(),
                    desc: "Pulsa Indosat Rp 25.000".into(),
                },
                PrepaidProduct {
                    product_name: "XL 10.000".into(),
                    category: "Pulsa".into(),
                    brand: "XL".into(),
                    kind: "Umum".into(),
                    seller_name: "DigiMock".into(),
                    price: 10550,
                    buyer_sku_code: "hxl10".into(),
                    buyer_product_status: true,
                    seller_product_status: true,
                    unlimited_stock: true,
                    stock: 999,
                    multi: true,
                    start_cut_off: "23:50".into(),
                    end_cut_off: "00:10".into(),
                    desc: "Pulsa XL Rp 10.000".into(),
                },
                PrepaidProduct {
                    product_name: "PLN 20.000".into(),
                    category: "PLN".into(),
                    brand: "PLN".into(),
                    kind: "Umum".into(),
                    seller_name: "DigiMock".into(),
                    price: 20300,
                    buyer_sku_code: "pln20".into(),
                    buyer_product_status: true,
                    seller_product_status: true,
                    unlimited_stock: true,
                    stock: 999,
                    multi: true,
                    start_cut_off: "23:50".into(),
                    end_cut_off: "00:10".into(),
                    desc: "Token PLN Rp 20.000".into(),
                },
                PrepaidProduct {
                    product_name: "PLN 50.000".into(),
                    category: "PLN".into(),
                    brand: "PLN".into(),
                    kind: "Umum".into(),
                    seller_name: "DigiMock".into(),
                    price: 50300,
                    buyer_sku_code: "pln50".into(),
                    buyer_product_status: true,
                    seller_product_status: true,
                    unlimited_stock: true,
                    stock: 999,
                    multi: true,
                    start_cut_off: "23:50".into(),
                    end_cut_off: "00:10".into(),
                    desc: "Token PLN Rp 50.000".into(),
                },
                PrepaidProduct {
                    product_name: "Mobile Legends 86 Diamonds".into(),
                    category: "Games".into(),
                    brand: "MOBILE LEGENDS".into(),
                    kind: "Umum".into(),
                    seller_name: "DigiMock".into(),
                    price: 21500,
                    buyer_sku_code: "ml86".into(),
                    buyer_product_status: true,
                    seller_product_status: true,
                    unlimited_stock: true,
                    stock: 999,
                    multi: true,
                    start_cut_off: "23:50".into(),
                    end_cut_off: "00:10".into(),
                    desc: "Mobile Legends 86 Diamonds".into(),
                },
                PrepaidProduct {
                    product_name: "Free Fire 140 Diamonds".into(),
                    category: "Games".into(),
                    brand: "FREE FIRE".into(),
                    kind: "Umum".into(),
                    seller_name: "DigiMock".into(),
                    price: 19500,
                    buyer_sku_code: "ff140".into(),
                    buyer_product_status: true,
                    seller_product_status: true,
                    unlimited_stock: true,
                    stock: 999,
                    multi: true,
                    start_cut_off: "23:50".into(),
                    end_cut_off: "00:10".into(),
                    desc: "Free Fire 140 Diamonds".into(),
                },
            ];
            PriceList::Prepaid(products)
        }
        PriceListKind::Pasca => {
            let products = vec![
                PostpaidProduct {
                    product_name: "PLN Pascabayar".into(),
                    category: "PLN PASCABAYAR".into(),
                    brand: "PLN PASCABAYAR".into(),
                    seller_name: "DigiMock".into(),
                    admin: 2500,
                    commission: 1500,
                    buyer_sku_code: "plnpas".into(),
                    buyer_product_status: true,
                    seller_product_status: true,
                    desc: "Tagihan Listrik PLN Pasca Bayar".into(),
                },
                PostpaidProduct {
                    product_name: "BPJS Kesehatan".into(),
                    category: "BPJS KESEHATAN".into(),
                    brand: "BPJS KESEHATAN".into(),
                    seller_name: "DigiMock".into(),
                    admin: 2500,
                    commission: 1000,
                    buyer_sku_code: "bpjs".into(),
                    buyer_product_status: true,
                    seller_product_status: true,
                    desc: "Tagihan Iuran BPJS Kesehatan".into(),
                },
            ];
            PriceList::Pasca(products)
        }
    }
}

/// Returns a simulated transaction result.
pub fn get_mock_transaction(req: &TransactionRequest) -> Transaction {
    let price = req.max_price.unwrap_or(20000);
    let mut raw = serde_json::Map::new();
    raw.insert("status".into(), json!("Sukses"));
    raw.insert("rc".into(), json!("00"));
    raw.insert("message".into(), json!("Transaksi Sukses (Mock Mode)"));
    raw.insert("sn".into(), json!("MOCK-SN-99887766"));

    Transaction {
        ref_id: req.ref_id.clone(),
        customer_no: req.customer_no.clone(),
        buyer_sku_code: req.sku.clone(),
        message: "Transaksi Sukses (Mock Mode)".into(),
        rc: "00".into(),
        sn: "MOCK-SN-99887766".into(),
        buyer_last_saldo: Some(4_980_000.0),
        price: Some(price),
        tele: None,
        wa: None,
        raw,
    }
}

/// Returns a simulated deposit ticket.
pub fn get_mock_deposit(req: &DepositRequest) -> DepositTicket {
    DepositTicket {
        rc: "00".into(),
        bank: req.bank.clone(),
        payment_method: "Bank Transfer".into(),
        account_no: "1234567890".into(),
        amount: req.amount + 123,
        notes: "Transfer nominal tepat sampai 3 digit terakhir (Mock Mode)".into(),
    }
}
