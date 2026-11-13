use serde::{Deserialize, Serialize};

use crate::de;

/// Which price list to fetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PriceListKind {
    /// Prepaid products (pulsa, data, games, ...).
    Prepaid,
    /// Postpaid products (PLN, PDAM, BPJS, ...).
    Pasca,
}

/// Command sent for postpaid transactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Command {
    /// Check the outstanding bill (`inq-pasca`).
    #[serde(rename = "inq-pasca")]
    Inquiry,
    /// Pay the outstanding bill (`pay-pasca`).
    #[serde(rename = "pay-pasca")]
    Pay,
    /// Check a previous transaction status (`status-pasca`).
    #[serde(rename = "status-pasca")]
    Status,
    /// Look up a PLN customer id (`pln-subscribe`).
    #[serde(rename = "pln-subscribe")]
    PlnSubscribe,
}

/// Creates a deposit ticket.
#[derive(Debug, Clone)]
pub struct DepositRequest {
    /// Amount to deposit.
    pub amount: i64,
    /// Destination bank, e.g. `BCA`, `MANDIRI`, `BRI`, `BNI`, `Flip`, `ShopeePay`.
    pub bank: String,
    /// Name of the bank account owner sending the transfer.
    pub owner_name: String,
}

/// Creates a prepaid or postpaid transaction.
#[derive(Debug, Clone)]
pub struct TransactionRequest {
    /// Product code (`buyer_sku_code`).
    pub sku: String,
    /// Customer number or id.
    pub customer_no: String,
    /// Your unique reference id.
    pub ref_id: String,
    /// Postpaid command; leave `None` for prepaid transactions.
    pub command: Option<Command>,
    /// Mark the transaction as a development/testing request.
    pub testing: bool,
    /// Maximum price accepted for the product.
    pub max_price: Option<i64>,
    /// Overrides the static webhook configured in your Digiflazz account.
    pub callback_url: Option<String>,
    /// Allow dots inside `customer_no`.
    pub allow_dot: Option<bool>,
}

impl TransactionRequest {
    /// Builds a prepaid transaction with the required fields.
    pub fn new(
        sku: impl Into<String>,
        customer_no: impl Into<String>,
        ref_id: impl Into<String>,
    ) -> Self {
        Self {
            sku: sku.into(),
            customer_no: customer_no.into(),
            ref_id: ref_id.into(),
            command: None,
            testing: false,
            max_price: None,
            callback_url: None,
            allow_dot: None,
        }
    }

    /// Builds a postpaid transaction that runs the given command.
    pub fn with_command(
        sku: impl Into<String>,
        customer_no: impl Into<String>,
        ref_id: impl Into<String>,
        command: Command,
    ) -> Self {
        Self {
            command: Some(command),
            ..Self::new(sku, customer_no, ref_id)
        }
    }
}

/// Response of [`crate::Digiflazz::cek_saldo`].
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Balance {
    /// Remaining deposit balance.
    #[serde(deserialize_with = "de::f64")]
    pub deposit: f64,
}

/// A prepaid product from [`crate::Digiflazz::daftar_harga`].
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PrepaidProduct {
    /// Product name.
    pub product_name: String,
    /// Product category.
    pub category: String,
    /// Product brand.
    pub brand: String,
    /// Product type.
    #[serde(rename = "type")]
    pub kind: String,
    /// Name of the seller providing the product.
    pub seller_name: String,
    /// Price set by the seller.
    #[serde(deserialize_with = "de::i64")]
    pub price: i64,
    /// Your product code.
    pub buyer_sku_code: String,
    /// Whether the product is active on your account.
    pub buyer_product_status: bool,
    /// Whether the product is active for the seller.
    pub seller_product_status: bool,
    /// Whether stock is unlimited.
    pub unlimited_stock: bool,
    /// Remaining seller stock; ignore when `unlimited_stock` is set.
    #[serde(deserialize_with = "de::i64")]
    pub stock: i64,
    /// Whether the same destination may be topped up more than once a day.
    pub multi: bool,
    /// Cut-off start time (`hh:mm`).
    pub start_cut_off: String,
    /// Cut-off end time (`hh:mm`).
    pub end_cut_off: String,
    /// Product description.
    pub desc: String,
}

/// A postpaid product from [`crate::Digiflazz::daftar_harga`].
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostpaidProduct {
    /// Product name.
    pub product_name: String,
    /// Product category.
    pub category: String,
    /// Product brand.
    pub brand: String,
    /// Name of the seller providing the product.
    pub seller_name: String,
    /// Admin fee charged for the product.
    #[serde(deserialize_with = "de::i64")]
    pub admin: i64,
    /// Commission you earn for the product.
    #[serde(deserialize_with = "de::i64")]
    pub commission: i64,
    /// Your product code.
    pub buyer_sku_code: String,
    /// Whether the product is active on your account.
    pub buyer_product_status: bool,
    /// Whether the product is active for the seller.
    pub seller_product_status: bool,
    /// Product description.
    pub desc: String,
}

/// Result of [`crate::Digiflazz::deposit`].
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DepositTicket {
    /// Digiflazz response code.
    #[serde(default)]
    pub rc: String,
    /// Destination bank of the transfer.
    #[serde(default)]
    pub bank: String,
    /// Payment method, either `Bank Transfer` or `Virtual Account`.
    #[serde(default)]
    pub payment_method: String,
    /// Destination account number.
    #[serde(default)]
    pub account_no: String,
    /// Final amount that must be transferred.
    #[serde(deserialize_with = "de::i64")]
    pub amount: i64,
    /// Remark that must be included in the transfer.
    #[serde(default)]
    pub notes: String,
}

/// Transaction status reported by Digiflazz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The transaction succeeded.
    Sukses,
    /// The transaction is still being processed.
    Pending,
    /// The transaction failed.
    Gagal,
    /// A status this crate does not recognise.
    Other,
}

impl Status {
    fn from_str(value: &str) -> Self {
        if value.eq_ignore_ascii_case("sukses") {
            Self::Sukses
        } else if value.eq_ignore_ascii_case("pending") {
            Self::Pending
        } else if value.eq_ignore_ascii_case("gagal") {
            Self::Gagal
        } else {
            Self::Other
        }
    }
}

/// Result of [`crate::Digiflazz::transaksi`].
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Transaction {
    /// Your reference id.
    #[serde(default)]
    pub ref_id: String,
    /// Customer number or id.
    #[serde(default)]
    pub customer_no: String,
    /// Your product code.
    #[serde(default)]
    pub buyer_sku_code: String,
    /// Human readable transaction message.
    #[serde(default)]
    pub message: String,
    /// Digiflazz response code.
    #[serde(default)]
    pub rc: String,
    /// Serial number, when the product provides one.
    #[serde(default)]
    pub sn: String,
    /// Remaining balance after the transaction.
    #[serde(default, deserialize_with = "de::opt_f64")]
    pub buyer_last_saldo: Option<f64>,
    /// Product price charged for the transaction.
    #[serde(default, deserialize_with = "de::opt_i64")]
    pub price: Option<i64>,
    /// Seller telegram contact, when provided.
    #[serde(default)]
    pub tele: Option<String>,
    /// Seller whatsapp contact, when provided.
    #[serde(default)]
    pub wa: Option<String>,
    /// Every field of the raw response, including postpaid-only keys such as
    /// `customer_name`, `admin`, `selling_price` and `desc`.
    #[serde(flatten)]
    pub raw: serde_json::Map<String, serde_json::Value>,
}

impl Transaction {
    /// Parsed value of the `status` field.
    pub fn status(&self) -> Status {
        self.raw
            .get("status")
            .and_then(serde_json::Value::as_str)
            .map(Status::from_str)
            .unwrap_or(Status::Other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepaid_product_accepts_string_numbers() {
        let product: PrepaidProduct = serde_json::from_str(
            r#"{
                "product_name": "Xl 100.000",
                "category": "Pulsa",
                "brand": "XL",
                "type": "Umum",
                "seller_name": "PT. ABC",
                "price": "98000",
                "buyer_sku_code": "X100",
                "buyer_product_status": true,
                "seller_product_status": true,
                "unlimited_stock": true,
                "stock": "0",
                "multi": true,
                "start_cut_off": "23:45",
                "end_cut_off": "00:15",
                "desc": "Pulsa Xl Rp 100.000"
            }"#,
        )
        .unwrap();

        assert_eq!(product.price, 98000);
        assert_eq!(product.stock, 0);
        assert_eq!(product.kind, "Umum");
    }

    #[test]
    fn transaction_reports_status() {
        let transaction: Transaction = serde_json::from_str(
            r#"{
                "ref_id": "some1d",
                "customer_no": "087800001233",
                "buyer_sku_code": "xld25",
                "message": "Transaksi Pending",
                "status": "Pending",
                "rc": "03",
                "sn": "",
                "buyer_last_saldo": 100000,
                "price": 25000,
                "tele": "@telegram",
                "wa": "081234512345"
            }"#,
        )
        .unwrap();

        assert_eq!(transaction.status(), Status::Pending);
        assert_eq!(transaction.price, Some(25000));
        assert_eq!(transaction.buyer_last_saldo, Some(100000.0));
        assert_eq!(transaction.tele.as_deref(), Some("@telegram"));
    }

    #[test]
    fn transaction_keeps_postpaid_fields() {
        let transaction: Transaction = serde_json::from_str(
            r#"{
                "status": "Sukses",
                "customer_name": "Nama Pelanggan",
                "admin": 2500,
                "selling_price": 11000
            }"#,
        )
        .unwrap();

        assert_eq!(transaction.status(), Status::Sukses);
        assert_eq!(transaction.raw["customer_name"], "Nama Pelanggan");
        assert_eq!(transaction.raw["selling_price"], 11000);
    }

    #[test]
    fn transaction_tolerates_missing_optional_fields() {
        let transaction: Transaction = serde_json::from_str(r#"{"status":"Sukses"}"#).unwrap();
        assert_eq!(transaction.status(), Status::Sukses);
        assert_eq!(transaction.price, None);
        assert_eq!(transaction.ref_id, "");
    }

    #[test]
    fn deposit_ticket_parses_numeric_string_amount() {
        let ticket: DepositTicket = serde_json::from_str(
            r#"{
                "rc": "00",
                "bank": "BCA",
                "payment_method": "Bank Transfer",
                "account_no": "0123 4567 89",
                "notes": "A6R5UPV",
                "amount": "10000001"
            }"#,
        )
        .unwrap();

        assert_eq!(ticket.amount, 10_000_001);
        assert_eq!(ticket.notes, "A6R5UPV");
    }
}
