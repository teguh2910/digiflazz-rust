//! Live smoke test against the Digiflazz API.
//!
//! Run with:
//!
//! ```sh
//! set -a; source .env; set +a
//! cargo run --example smoke
//! ```
//!
//! Uses `DIGIFLAZZ_DEVELOPMENT_KEY` unless `DIGIFLAZZ_ENV=production`, in which
//! case it uses `DIGIFLAZZ_PRODUCTION_KEY`. The prepaid transaction always sets
//! `testing: true`, and a failing step (rate limits, empty balance) is reported
//! without aborting the remaining steps.

use digiflazz::{
    Command, Digiflazz, Error, PriceList, PriceListKind, Transaction, TransactionRequest,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let username = std::env::var("DIGIFLAZZ_USERNAME").expect("DIGIFLAZZ_USERNAME is not set");

    let (key, production) = match std::env::var("DIGIFLAZZ_ENV").as_deref() {
        Ok("production") => (
            std::env::var("DIGIFLAZZ_PRODUCTION_KEY").expect("DIGIFLAZZ_PRODUCTION_KEY is not set"),
            true,
        ),
        _ => (
            std::env::var("DIGIFLAZZ_DEVELOPMENT_KEY")
                .expect("DIGIFLAZZ_DEVELOPMENT_KEY is not set"),
            false,
        ),
    };

    println!(
        "environment: {} (username: {username})",
        if production {
            "production"
        } else {
            "development"
        }
    );

    let client = Digiflazz::new(&username, &key)?;

    match client.cek_saldo().await {
        Ok(balance) => println!("cek-saldo  -> deposit = {}", balance.deposit),
        Err(error) => println!("cek-saldo  -> {error}"),
    }

    for kind in [PriceListKind::Prepaid, PriceListKind::Pasca] {
        let label = match kind {
            PriceListKind::Prepaid => "prepaid",
            PriceListKind::Pasca => "pasca  ",
        };

        match client.daftar_harga(kind).await {
            Ok(PriceList::Prepaid(products)) => {
                println!("price-list -> {label}: {} products", products.len());
                for product in products.iter().take(3) {
                    println!("             {} {}", product.buyer_sku_code, product.price);
                }
            }
            Ok(PriceList::Pasca(products)) => {
                println!("price-list -> {label}: {} products", products.len());
                for product in products.iter().take(3) {
                    println!(
                        "             {} admin={}",
                        product.buyer_sku_code, product.admin
                    );
                }
            }
            Err(error) => println!("price-list -> {label}: {error}"),
        }
    }

    let mut prepaid = TransactionRequest::new("xld10", "087800001230", "smoke-prepaid-1");
    prepaid.testing = true;
    report("transaksi ", client.transaksi(&prepaid).await);

    let postpaid =
        TransactionRequest::with_command("pln", "530000000001", "smoke-pasca-1", Command::Inquiry);
    report("inq-pasca ", client.transaksi(&postpaid).await);

    Ok(())
}

fn report(label: &str, result: Result<Transaction, Error>) {
    match result {
        Ok(transaction) => println!(
            "{label} -> {:?} rc={} sn={:?} price={:?} message={}",
            transaction.status(),
            transaction.rc,
            transaction.sn,
            transaction.price,
            transaction.message
        ),
        Err(error) => println!("{label} -> {error}"),
    }
}
