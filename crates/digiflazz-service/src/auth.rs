//! Bearer token authentication.

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;

use crate::error::ApiError;
use crate::state::AppState;

/// Marks a request that presented a valid `Authorization: Bearer <token>`.
pub struct Authenticated;

impl FromRequestParts<AppState> for Authenticated {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(ApiError::unauthorized)?;

        let token = header
            .strip_prefix("Bearer ")
            .or_else(|| header.strip_prefix("bearer "))
            .map(str::trim)
            .ok_or_else(ApiError::unauthorized)?;

        if !constant_time_eq(token.as_bytes(), state.config().api_token.as_bytes()) {
            return Err(ApiError::unauthorized());
        }

        Ok(Authenticated)
    }
}

/// Compares two byte strings without short-circuiting on the first difference.
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    let mut difference = 0u8;
    for (a, b) in left.iter().zip(right) {
        difference |= a ^ b;
    }

    difference == 0
}

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn matches_equal_values() {
        assert!(constant_time_eq(b"secret", b"secret"));
    }

    #[test]
    fn rejects_different_values() {
        assert!(!constant_time_eq(b"secret", b"another"));
        assert!(!constant_time_eq(b"secret", b"secret "));
        assert!(!constant_time_eq(b"", b"secret"));
    }
}
