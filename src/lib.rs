//! Unofficial async Rust client for the [Digiflazz](https://digiflazz.com) buyer API.
//!
//! ```no_run
//! use digiflazz::{Digiflazz, TransactionRequest};
//!
//! # async fn run() -> Result<(), digiflazz::Error> {
//! let client = Digiflazz::new("username", "api-key")?;
//!
//! let balance = client.cek_saldo().await?;
//! println!("deposit: {}", balance.deposit);
//!
//! let transaction = client
//!     .transaksi(&TransactionRequest::new("xld25", "087800001233", "INV-1"))
//!     .await?;
//! println!("{}: {:?}", transaction.message, transaction.status());
//! # Ok(())
//! # }
//! ```

#![deny(missing_docs)]

mod client;
mod de;
mod error;
mod models;

pub use client::{Builder, Digiflazz, PriceList};
pub use error::Error;
pub use models::{
    Balance, Command, DepositRequest, DepositTicket, PostpaidProduct, PrepaidProduct,
    PriceListKind, Status, Transaction, TransactionRequest,
};
