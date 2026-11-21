//! `POST /deposit`.

use axum::extract::State;
use axum::Json;
use digiflazz::{DepositRequest, DepositTicket};
use serde::Deserialize;

use crate::auth::Authenticated;
use crate::error::ApiError;
use crate::json::JsonBody;
use crate::state::AppState;

/// Request body for creating a deposit ticket.
#[derive(Debug, Deserialize)]
pub struct DepositBody {
    /// Amount to deposit.
    amount: i64,
    /// Destination bank.
    bank: String,
    /// Bank account owner name.
    #[serde(alias = "name")]
    owner_name: String,
}

/// Creates a deposit ticket.
pub async fn deposit(
    _auth: Authenticated,
    State(state): State<AppState>,
    JsonBody(body): JsonBody<DepositBody>,
) -> Result<Json<DepositTicket>, ApiError> {
    let request = DepositRequest {
        amount: body.amount,
        bank: body.bank,
        owner_name: body.owner_name,
    };

    tracing::info!(
        amount = request.amount,
        bank = %request.bank,
        "creating deposit ticket"
    );

    if state.config().mock_mode {
        return Ok(Json(crate::mock::get_mock_deposit(&request)));
    }

    let ticket = state.client().deposit(&request).await?;
    Ok(Json(ticket))
}
