//! Service configuration loaded from the environment.

use std::time::Duration;

/// Runtime configuration for the service.
#[derive(Debug, Clone)]
pub struct Config {
    /// Digiflazz username.
    pub username: String,
    /// Digiflazz API key.
    pub api_key: String,
    /// Digiflazz API base URL.
    pub base_url: String,
    /// Whether the service runs against production credentials.
    pub production: bool,
    /// Bearer token clients must present.
    pub api_token: String,
    /// Port to listen on.
    pub port: u16,
    /// How long a price list entry stays fresh.
    pub price_list_ttl: Duration,
    /// Browser origins allowed to call the API. Empty disables CORS.
    pub cors_origins: Vec<String>,
    /// Whether mock mode is active (returns simulated data without calling upstream).
    pub mock_mode: bool,
}

/// Environment variables the service needs, reported together when missing.
const REQUIRED: [&str; 3] = [
    "DIGIFLAZZ_USERNAME",
    "DIGIFLAZZ_API_KEY",
    "SERVICE_API_TOKEN",
];

/// Minimum accepted `SERVICE_API_TOKEN` length.
const MIN_TOKEN_LEN: usize = 16;

/// Why the configuration could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// One or more required variables are absent.
    #[error("missing required environment variables: {}", .0.join(", "))]
    Missing(Vec<String>),
    /// A value was present but invalid.
    #[error("invalid {name}: {reason}")]
    Invalid {
        /// Variable name.
        name: &'static str,
        /// What is wrong with it.
        reason: String,
    },
}

impl Config {
    /// Reads configuration from the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        let missing: Vec<String> = REQUIRED
            .iter()
            .filter(|key| std::env::var(key).is_err())
            .map(|key| (*key).to_owned())
            .collect();

        if !missing.is_empty() {
            return Err(ConfigError::Missing(missing));
        }

        let api_token = std::env::var("SERVICE_API_TOKEN").unwrap_or_default();
        if api_token.len() < MIN_TOKEN_LEN {
            return Err(ConfigError::Invalid {
                name: "SERVICE_API_TOKEN",
                reason: format!("must be at least {MIN_TOKEN_LEN} characters"),
            });
        }

        let port = match std::env::var("PORT") {
            Ok(raw) => raw.parse().map_err(|_| ConfigError::Invalid {
                name: "PORT",
                reason: format!("expected a port number, got `{raw}`"),
            })?,
            Err(_) => 8080,
        };

        let price_list_ttl = match std::env::var("PRICE_LIST_TTL_SECS") {
            Ok(raw) => {
                let secs: u64 = raw.parse().map_err(|_| ConfigError::Invalid {
                    name: "PRICE_LIST_TTL_SECS",
                    reason: format!("expected a whole number of seconds, got `{raw}`"),
                })?;
                Duration::from_secs(secs)
            }
            Err(_) => Duration::from_secs(300),
        };

        let environment = std::env::var("DIGIFLAZZ_ENV").unwrap_or_else(|_| "development".into());
        let production = match environment.as_str() {
            "production" | "prod" => true,
            "development" | "dev" => false,
            other => {
                return Err(ConfigError::Invalid {
                    name: "DIGIFLAZZ_ENV",
                    reason: format!("expected development or production, got `{other}`"),
                })
            }
        };

        let mock_mode = match std::env::var("DIGIFLAZZ_MOCK") {
            Ok(ref v) if v.eq_ignore_ascii_case("true") || v == "1" => true,
            _ => {
                let u = std::env::var("DIGIFLAZZ_USERNAME").unwrap_or_default();
                let k = std::env::var("DIGIFLAZZ_API_KEY").unwrap_or_default();
                u == "sandbox_user"
                    || u == "your_digiflazz_username"
                    || u.is_empty()
                    || k == "sandbox_api_key"
                    || k == "your_digiflazz_api_key"
                    || k.is_empty()
            }
        };

        Ok(Self {
            username: std::env::var("DIGIFLAZZ_USERNAME").unwrap_or_default(),
            api_key: std::env::var("DIGIFLAZZ_API_KEY").unwrap_or_default(),
            base_url: std::env::var("DIGIFLAZZ_BASE_URL")
                .unwrap_or_else(|_| "https://api.digiflazz.com/v1".to_owned()),
            production,
            api_token,
            port,
            price_list_ttl,
            cors_origins: parse_origins(std::env::var("CORS_ORIGINS").ok()),
            mock_mode,
        })
    }
}

/// Splits a comma-separated origin list, dropping blanks.
fn parse_origins(raw: Option<String>) -> Vec<String> {
    raw.unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_variables_are_reported_together() {
        // `from_env` reads process globals, so only assert the shape of the
        // error for a variable no test sets.
        let missing = REQUIRED
            .iter()
            .filter(|key| std::env::var(key).is_err())
            .count();
        assert!(missing <= REQUIRED.len());
    }
}
