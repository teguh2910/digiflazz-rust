//! `GET /balance`.

use axum::extract::State;
use axum::Json;
use digiflazz::Balance;

use crate::auth::Authenticated;
use crate::error::ApiError;
use crate::state::AppState;

/// Returns the remaining deposit balance.
pub async fn balance(
    _auth: Authenticated,
    State(state): State<AppState>,
) -> Result<Json<Balance>, ApiError> {
    if state.config().mock_mode {
        return Ok(Json(crate::mock::get_mock_balance()));
    }
    let balance = state.client().cek_saldo().await?;
    Ok(Json(balance))
}
