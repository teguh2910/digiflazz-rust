//! A JSON body extractor that reports failures as JSON.

use axum::extract::{FromRequest, Request};
use axum::Json;
use serde::de::DeserializeOwned;

use crate::error::ApiError;

/// Deserializes a JSON body, converting rejections into [`ApiError`].
///
/// Axum's built-in extractor answers with plain text, which breaks clients that
/// expect every response to be JSON.
pub struct JsonBody<T>(pub T);

impl<T, S> FromRequest<S> for JsonBody<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(request, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(ApiError::bad_request(rejection.body_text())),
        }
    }
}
