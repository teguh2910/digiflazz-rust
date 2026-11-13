//! `GET /price-list?kind=prepaid|pasca`.

use axum::extract::{Query, State};
use axum::Json;
use digiflazz::{PriceList, PriceListKind};
use serde::{Deserialize, Serialize};

use crate::auth::Authenticated;
use crate::error::ApiError;
use crate::state::AppState;

/// Query string for the price list endpoint.
#[derive(Debug, Deserialize)]
pub struct PriceListQuery {
    kind: Option<String>,
}

/// Price list response, keeping the requested kind alongside the products.
#[derive(Debug, Serialize)]
pub struct PriceListResponse {
    /// `prepaid` or `pasca`.
    pub kind: &'static str,
    /// Products for the requested kind.
    pub products: PriceList,
}

/// Returns the price list, served from cache when fresh.
pub async fn price_list(
    _auth: Authenticated,
    State(state): State<AppState>,
    Query(query): Query<PriceListQuery>,
) -> Result<Json<PriceListResponse>, ApiError> {
    let kind = match query.kind.as_deref() {
        None | Some("prepaid") => PriceListKind::Prepaid,
        Some("pasca") | Some("postpaid") => PriceListKind::Pasca,
        Some(other) => return Err(ApiError::unknown_kind(other)),
    };

    let client = state.client();
    let products = state.cache().get(kind, client.daftar_harga(kind)).await?;

    let kind = match kind {
        PriceListKind::Prepaid => "prepaid",
        PriceListKind::Pasca => "pasca",
    };

    Ok(Json(PriceListResponse { kind, products }))
}
