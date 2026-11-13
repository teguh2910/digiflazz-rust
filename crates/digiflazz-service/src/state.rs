//! Shared application state.

use std::sync::Arc;

use digiflazz::Digiflazz;

use crate::cache::PriceListCache;
use crate::config::Config;

/// State cloned into every request handler.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    client: Digiflazz,
    cache: PriceListCache,
    config: Config,
}

impl AppState {
    /// Builds the state from configuration.
    pub fn new(config: Config) -> Result<Self, digiflazz::Error> {
        let client = Digiflazz::with_base_url(
            config.username.clone(),
            config.api_key.clone(),
            config.base_url.clone(),
        )?;

        Ok(Self {
            inner: Arc::new(Inner {
                client,
                cache: PriceListCache::new(config.price_list_ttl),
                config,
            }),
        })
    }

    /// The Digiflazz client.
    pub fn client(&self) -> &Digiflazz {
        &self.inner.client
    }

    /// The price list cache.
    pub fn cache(&self) -> &PriceListCache {
        &self.inner.cache
    }

    /// The service configuration.
    pub fn config(&self) -> &Config {
        &self.inner.config
    }
}
