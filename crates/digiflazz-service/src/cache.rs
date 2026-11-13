//! TTL cache for the price list.
//!
//! Digiflazz rate-limits the price list endpoint (`rc 83`), so callers should
//! not hit it per request. The cache is per-process and keeps serving a stale
//! entry when the upstream call fails, as long as one exists.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use digiflazz::{PriceList, PriceListKind};
use tokio::sync::{Mutex, RwLock};

struct Entry {
    list: PriceList,
    expires_at: Instant,
}

/// Caches `PriceList` per kind for a fixed TTL.
#[derive(Clone)]
pub struct PriceListCache {
    ttl: Duration,
    entries: Arc<RwLock<HashMap<PriceListKind, Entry>>>,
    /// One lock per kind so concurrent misses issue a single upstream call.
    locks: Arc<HashMap<PriceListKind, Arc<Mutex<()>>>>,
}

impl PriceListCache {
    /// Creates a cache whose entries live for `ttl`.
    pub fn new(ttl: Duration) -> Self {
        let locks = [PriceListKind::Prepaid, PriceListKind::Pasca]
            .into_iter()
            .map(|kind| (kind, Arc::new(Mutex::new(()))))
            .collect();

        Self {
            ttl,
            entries: Arc::new(RwLock::new(HashMap::new())),
            locks: Arc::new(locks),
        }
    }

    /// Returns a cached list, loading it through `load` when absent or expired.
    ///
    /// If `load` fails but a previously cached entry exists, that entry is
    /// returned instead so a transient upstream failure does not break callers.
    pub async fn get(
        &self,
        kind: PriceListKind,
        load: impl Future<Output = Result<PriceList, digiflazz::Error>>,
    ) -> Result<PriceList, digiflazz::Error> {
        if let Some(list) = self.fresh(kind).await {
            return Ok(list);
        }

        let lock = self.locks[&kind].clone();
        let _guard = lock.lock().await;

        // Another task may have filled the cache while we waited.
        if let Some(list) = self.fresh(kind).await {
            return Ok(list);
        }

        match load.await {
            Ok(list) => {
                let mut entries = self.entries.write().await;
                entries.insert(
                    kind,
                    Entry {
                        list: list.clone(),
                        expires_at: Instant::now() + self.ttl,
                    },
                );
                Ok(list)
            }
            Err(error) => match self.entries.read().await.get(&kind) {
                Some(entry) => {
                    tracing::warn!(
                        ?kind,
                        error = %error,
                        "serving stale price list after upstream failure"
                    );
                    Ok(entry.list.clone())
                }
                None => Err(error),
            },
        }
    }

    async fn fresh(&self, kind: PriceListKind) -> Option<PriceList> {
        let entries = self.entries.read().await;
        let entry = entries.get(&kind)?;

        if entry.expires_at > Instant::now() {
            Some(entry.list.clone())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn sample(kind: PriceListKind) -> PriceList {
        match kind {
            PriceListKind::Prepaid => PriceList::Prepaid(Vec::new()),
            PriceListKind::Pasca => PriceList::Pasca(Vec::new()),
        }
    }

    #[tokio::test]
    async fn serves_from_cache_within_ttl() {
        let cache = PriceListCache::new(Duration::from_secs(60));
        let calls = AtomicUsize::new(0);

        for _ in 0..2 {
            cache
                .get(PriceListKind::Prepaid, async {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(sample(PriceListKind::Prepaid))
                })
                .await
                .unwrap();
        }

        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn reloads_after_ttl_expires() {
        let cache = PriceListCache::new(Duration::from_secs(0));
        let calls = AtomicUsize::new(0);

        for _ in 0..2 {
            cache
                .get(PriceListKind::Prepaid, async {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(sample(PriceListKind::Prepaid))
                })
                .await
                .unwrap();
        }

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn serves_stale_entry_when_reload_fails() {
        let cache = PriceListCache::new(Duration::from_secs(0));

        cache
            .get(PriceListKind::Prepaid, async {
                Ok(sample(PriceListKind::Prepaid))
            })
            .await
            .unwrap();

        let result = cache
            .get(PriceListKind::Prepaid, async {
                Err(digiflazz::Error::Rejected {
                    rc: "83".to_owned(),
                    message: "limit".to_owned(),
                })
            })
            .await;

        assert!(result.is_ok(), "stale entry should be served");
    }

    #[tokio::test]
    async fn surfaces_the_error_when_nothing_is_cached() {
        let cache = PriceListCache::new(Duration::from_secs(0));

        let result = cache
            .get(PriceListKind::Pasca, async {
                Err(digiflazz::Error::Rejected {
                    rc: "83".to_owned(),
                    message: "limit".to_owned(),
                })
            })
            .await;

        assert!(result.is_err());
    }
}
