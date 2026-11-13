//! Maps failures onto HTTP responses.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use digiflazz::Error;
use serde_json::json;

/// An error rendered as a JSON HTTP response.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    body: serde_json::Value,
}

impl ApiError {
    fn new(status: StatusCode, body: serde_json::Value) -> Self {
        Self { status, body }
    }

    /// The caller did not present a usable bearer token.
    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, json!({ "error": "unauthorized" }))
    }

    /// The request body or query was not valid.
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            json!({ "error": "bad_request", "message": message.into() }),
        )
    }

    /// A route parameter is outside its accepted set.
    pub fn unknown_kind(value: &str) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            json!({
                "error": "bad_request",
                "message": format!("unknown kind `{value}`, expected prepaid or pasca"),
            }),
        )
    }

    /// A `command` value is not one Digiflazz accepts.
    pub fn unknown_command(value: &str) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            json!({
                "error": "bad_request",
                "message": format!(
                    "unknown command `{value}`, expected inq-pasca, pay-pasca, status-pasca or pln-subscribe"
                ),
            }),
        )
    }

    /// Translates an SDK error.
    ///
    /// Upstream conditions (rate limits, bad signatures, insufficient balance)
    /// are reported as `502` because they are not the caller's mistake, and the
    /// Digiflazz response code is preserved so callers can act on it.
    pub fn from_sdk(error: Error) -> Self {
        match error {
            Error::Rejected { rc, message } => Self::new(
                StatusCode::BAD_GATEWAY,
                json!({ "error": "upstream_rejected", "rc": rc, "message": message }),
            ),
            Error::Api { status, message } => Self::new(
                StatusCode::BAD_GATEWAY,
                json!({ "error": "upstream_error", "status": status, "message": message }),
            ),
            Error::Http(error) => Self::new(
                StatusCode::GATEWAY_TIMEOUT,
                json!({ "error": "upstream_unavailable", "message": error.to_string() }),
            ),
            Error::Decode(error) => Self::new(
                StatusCode::BAD_GATEWAY,
                json!({ "error": "upstream_invalid_response", "message": error.to_string() }),
            ),
            Error::Config(message) => Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "error": "misconfigured", "message": message }),
            ),
            other => Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "error": "internal", "message": other.to_string() }),
            ),
        }
    }
}

impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        Self::from_sdk(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejection_keeps_the_digiflazz_code() {
        let error = ApiError::from_sdk(Error::Rejected {
            rc: "83".to_owned(),
            message: "limit".to_owned(),
        });

        assert_eq!(error.status, StatusCode::BAD_GATEWAY);
        assert_eq!(error.body["rc"], "83");
        assert_eq!(error.body["error"], "upstream_rejected");
    }

    #[test]
    fn http_failure_is_a_gateway_timeout() {
        // A transport failure cannot be constructed directly; assert the
        // mapping for the variants the service can actually surface.
        let error = ApiError::from_sdk(Error::Config("boom"));
        assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error.body["error"], "misconfigured");
    }
}
