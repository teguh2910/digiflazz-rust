//! `POST /transaction`.

use axum::extract::State;
use axum::Json;
use digiflazz::{Command, Transaction, TransactionRequest};
use serde::Deserialize;

use crate::auth::Authenticated;
use crate::error::ApiError;
use crate::json::JsonBody;
use crate::state::AppState;

/// Request body for creating a transaction.
#[derive(Debug, Deserialize)]
pub struct TransactionBody {
    /// Product code.
    sku: String,
    /// Customer number or id.
    customer_no: String,
    /// Caller's unique reference id.
    ref_id: String,
    /// Postpaid command.
    #[serde(default)]
    command: Option<String>,
    /// Development flag; defaults per environment when omitted.
    #[serde(default)]
    testing: Option<bool>,
    /// Price ceiling.
    #[serde(default)]
    max_price: Option<i64>,
    /// Per-request callback override.
    #[serde(default, alias = "cb_url")]
    callback_url: Option<String>,
    /// Allow dots in `customer_no`.
    #[serde(default)]
    allow_dot: Option<bool>,
}

/// Creates a prepaid or postpaid transaction.
pub async fn transaction(
    _auth: Authenticated,
    State(state): State<AppState>,
    JsonBody(body): JsonBody<TransactionBody>,
) -> Result<Json<Transaction>, ApiError> {
    let command = match body.command.as_deref() {
        None => None,
        Some("inq-pasca") => Some(Command::Inquiry),
        Some("pay-pasca") => Some(Command::Pay),
        Some("status-pasca") => Some(Command::Status),
        Some("pln-subscribe") => Some(Command::PlnSubscribe),
        Some(other) => return Err(ApiError::unknown_command(other)),
    };

    let testing = resolve_testing(state.config().production, body.testing, &body.ref_id);

    let mut request = TransactionRequest::new(body.sku, body.customer_no, body.ref_id);
    request.command = command;
    request.testing = testing;
    request.max_price = body.max_price;
    request.callback_url = body.callback_url;
    request.allow_dot = body.allow_dot;

    tracing::info!(
        sku = %request.sku,
        ref_id = %request.ref_id,
        command = ?request.command,
        testing = request.testing,
        "creating transaction"
    );

    if state.config().mock_mode {
        return Ok(Json(crate::mock::get_mock_transaction(&request)));
    }

    let transaction = state.client().transaksi(&request).await?;
    Ok(Json(transaction))
}

/// Decides the effective `testing` flag.
///
/// In development an omitted flag defaults to `true`, so an accidental call
/// does not move real balance. In production it keeps the Digiflazz default
/// (`false`) and warns, because that call spends real money.
fn resolve_testing(production: bool, requested: Option<bool>, ref_id: &str) -> bool {
    match (production, requested) {
        (false, None) => true,
        (true, None) => {
            tracing::warn!(
                ref_id = %ref_id,
                "production transaction without an explicit `testing` flag"
            );
            false
        }
        (_, Some(value)) => value,
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_testing;

    #[test]
    fn development_defaults_to_testing() {
        assert!(resolve_testing(false, None, "ref"));
    }

    #[test]
    fn production_defaults_to_real_transactions() {
        assert!(!resolve_testing(true, None, "ref"));
    }

    #[test]
    fn explicit_values_win() {
        assert!(!resolve_testing(false, Some(false), "ref"));
        assert!(resolve_testing(true, Some(true), "ref"));
    }
}
