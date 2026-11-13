use md5::{Digest, Md5};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::Error;
use crate::models::{
    Balance, Command, DepositRequest, DepositTicket, PostpaidProduct, PrepaidProduct,
    PriceListKind, Transaction, TransactionRequest,
};

const DEFAULT_BASE_URL: &str = "https://api.digiflazz.com/v1";

/// Price list response, matching the requested [`PriceListKind`].
#[derive(Debug, Clone)]
pub enum PriceList {
    /// Products returned for [`PriceListKind::Prepaid`].
    Prepaid(Vec<PrepaidProduct>),
    /// Products returned for [`PriceListKind::Pasca`].
    Pasca(Vec<PostpaidProduct>),
}

#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
}

#[derive(Serialize)]
struct BalancePayload<'a> {
    cmd: &'static str,
    username: &'a str,
    sign: String,
}

#[derive(Serialize)]
struct PriceListPayload<'a> {
    cmd: PriceListKind,
    username: &'a str,
    sign: String,
}

#[derive(Serialize)]
struct DepositPayload<'a> {
    username: &'a str,
    amount: i64,
    bank: &'a str,
    owner_name: &'a str,
    sign: String,
}

#[derive(Serialize)]
struct TransactionPayload<'a> {
    username: &'a str,
    buyer_sku_code: &'a str,
    customer_no: &'a str,
    ref_id: &'a str,
    sign: String,
    testing: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    commands: Option<Command>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_price: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cb_url: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allow_dot: Option<bool>,
}

/// Async client for the Digiflazz buyer API.
///
/// Cloning is cheap: the inner HTTP client is shared.
#[derive(Debug, Clone)]
pub struct Digiflazz {
    username: String,
    key: String,
    base_url: String,
    http: reqwest::Client,
}

impl Digiflazz {
    /// Creates a client for the production Digiflazz endpoint.
    pub fn new(username: impl Into<String>, key: impl Into<String>) -> Result<Self, Error> {
        Self::with_base_url(username, key, DEFAULT_BASE_URL)
    }

    /// Creates a client pointing at a custom endpoint (sandbox or tests).
    pub fn with_base_url(
        username: impl Into<String>,
        key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Result<Self, Error> {
        Self::builder(username, key, base_url).build()
    }

    /// Starts configuring a client with a custom HTTP client.
    pub fn builder(
        username: impl Into<String>,
        key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Builder {
        Builder {
            username: username.into(),
            key: key.into(),
            base_url: base_url.into(),
            http: None,
        }
    }

    fn sign(&self, suffix: &str) -> String {
        let mut hasher = Md5::new();
        hasher.update(self.username.as_bytes());
        hasher.update(self.key.as_bytes());
        hasher.update(suffix.as_bytes());
        hex::encode(hasher.finalize())
    }

    async fn post_value(&self, path: &str, body: &impl Serialize) -> Result<Value, Error> {
        let response = self
            .http
            .post(format!("{}{}", self.base_url.trim_end_matches('/'), path))
            .json(body)
            .send()
            .await?;

        let status = response.status();
        let bytes = response.bytes().await?;

        if !status.is_success() {
            return Err(Error::api(status.as_u16(), &bytes));
        }

        let envelope: Envelope<Value> = serde_json::from_slice(&bytes)?;

        if let Some(error) = Error::rejected(&envelope.data) {
            return Err(error);
        }

        Ok(envelope.data)
    }

    async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T, Error> {
        let data = self.post_value(path, body).await?;
        Ok(serde_json::from_value(data)?)
    }

    /// Checks the remaining deposit balance.
    pub async fn cek_saldo(&self) -> Result<Balance, Error> {
        let payload = BalancePayload {
            cmd: "deposit",
            username: &self.username,
            sign: self.sign("depo"),
        };

        self.post("/cek-saldo", &payload).await
    }

    /// Fetches the price list for prepaid or postpaid products.
    ///
    /// An account with no products of the requested kind yields an empty list;
    /// Digiflazz encodes that as `data: null`.
    pub async fn daftar_harga(&self, kind: PriceListKind) -> Result<PriceList, Error> {
        let payload = PriceListPayload {
            cmd: kind,
            username: &self.username,
            sign: self.sign("pricelist"),
        };

        let data = self.post_value("/price-list", &payload).await?;
        let products = if data.is_null() { json!([]) } else { data };

        match kind {
            PriceListKind::Prepaid => Ok(PriceList::Prepaid(serde_json::from_value(products)?)),
            PriceListKind::Pasca => Ok(PriceList::Pasca(serde_json::from_value(products)?)),
        }
    }

    /// Creates a deposit ticket.
    pub async fn deposit(&self, request: &DepositRequest) -> Result<DepositTicket, Error> {
        let payload = DepositPayload {
            username: &self.username,
            amount: request.amount,
            bank: &request.bank,
            owner_name: &request.owner_name,
            sign: self.sign("deposit"),
        };

        self.post("/deposit", &payload).await
    }

    /// Creates a prepaid or postpaid transaction.
    pub async fn transaksi(&self, request: &TransactionRequest) -> Result<Transaction, Error> {
        let payload = TransactionPayload {
            username: &self.username,
            buyer_sku_code: &request.sku,
            customer_no: &request.customer_no,
            ref_id: &request.ref_id,
            sign: self.sign(&request.ref_id),
            testing: request.testing,
            commands: request.command,
            max_price: request.max_price,
            cb_url: request.callback_url.as_deref(),
            allow_dot: request.allow_dot,
        };

        self.post("/transaction", &payload).await
    }
}

/// Configures a [`Digiflazz`] client.
#[derive(Debug)]
pub struct Builder {
    username: String,
    key: String,
    base_url: String,
    http: Option<reqwest::Client>,
}

impl Builder {
    /// Overrides the base URL.
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Uses a preconfigured HTTP client (proxies, timeouts, ...).
    pub fn http_client(mut self, http: reqwest::Client) -> Self {
        self.http = Some(http);
        self
    }

    /// Builds the client.
    pub fn build(self) -> Result<Digiflazz, Error> {
        if self.username.is_empty() {
            return Err(Error::Config("username must not be empty"));
        }
        if self.key.is_empty() {
            return Err(Error::Config("key must not be empty"));
        }

        Ok(Digiflazz {
            username: self.username,
            key: self.key,
            base_url: self.base_url,
            http: self.http.unwrap_or_default(),
        })
    }
}
