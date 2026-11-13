use serde_json::Value;

/// Errors returned by the Digiflazz client.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The API answered with a non-success HTTP status.
    #[error("request failed with status {status}: {message}")]
    Api {
        /// HTTP status code of the response.
        status: u16,
        /// Message extracted from the response body.
        message: String,
    },

    /// The request could not be completed (connection, TLS, timeout, ...).
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// Digiflazz answered with HTTP 200 but a non-success `rc`.
    ///
    /// The API reports logical failures (invalid signature, insufficient
    /// balance, price list rate limit, ...) this way instead of using an HTTP
    /// error status.
    #[error("request rejected (rc {rc}): {message}")]
    Rejected {
        /// Digiflazz response code.
        rc: String,
        /// Human readable reason reported by Digiflazz.
        message: String,
    },

    /// The response body did not match the expected shape.
    #[error("failed to decode response: {0}")]
    Decode(#[from] serde_json::Error),

    /// The client was built with invalid credentials.
    #[error("invalid configuration: {0}")]
    Config(&'static str),
}

/// Digiflazz response code meaning "success".
const RC_SUCCESS: &str = "00";

impl Error {
    pub(crate) fn api(status: u16, body: &[u8]) -> Self {
        let message = extract_message(&String::from_utf8_lossy(body));
        Self::Api { status, message }
    }

    /// Detects a logical rejection inside a successful HTTP response.
    ///
    /// Success payloads either omit `rc` (`cek-saldo`) or set it to `"00"`
    /// (`deposit`, `transaction`), so any other code means the request failed.
    pub(crate) fn rejected(data: &Value) -> Option<Self> {
        let rc = data.get("rc")?.as_str()?;
        if rc == RC_SUCCESS {
            return None;
        }

        let message = data
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Unknown error occurred");

        Some(Self::Rejected {
            rc: rc.to_owned(),
            message: message.to_owned(),
        })
    }
}

fn extract_message(body: &str) -> String {
    const UNKNOWN: &str = "Unknown error occurred";

    match serde_json::from_str::<Value>(body) {
        Ok(Value::String(message)) => message,
        Ok(Value::Object(map)) => map
            .get("message")
            .or_else(|| map.get("data").and_then(|data| data.get("message")))
            .and_then(Value::as_str)
            .unwrap_or(UNKNOWN)
            .to_owned(),
        Ok(_) => UNKNOWN.to_owned(),
        Err(_) if body.trim().is_empty() => UNKNOWN.to_owned(),
        Err(_) => body.trim().to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::extract_message;

    #[test]
    fn extracts_top_level_message() {
        assert_eq!(extract_message(r#"{"message":"nope"}"#), "nope");
    }

    #[test]
    fn extracts_nested_message() {
        assert_eq!(
            extract_message(r#"{"data":{"rc":"40","message":"saldo tidak cukup"}}"#),
            "saldo tidak cukup"
        );
    }

    #[test]
    fn falls_back_to_plain_body() {
        assert_eq!(extract_message("Bad Gateway"), "Bad Gateway");
    }

    #[test]
    fn falls_back_to_unknown() {
        assert_eq!(extract_message(r#"{"rc":"40"}"#), "Unknown error occurred");
        assert_eq!(extract_message("  "), "Unknown error occurred");
    }
}
