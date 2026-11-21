//! TTL cache for the price list.
//!
//! Digiflazz rate-limits the price list endpoint (`rc 83`), so callers should
//! not hit it per request. The cache is per-process, persists snapshots to disk,
//! and keeps serving stale entries when upstream fails or rate limits.

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use digiflazz::{PostpaidProduct, PrepaidProduct, PriceList, PriceListKind};
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
    /// Upstream error cooldowns per kind to avoid hammering after rejection (e.g. RC 83).
    cooldowns: Arc<RwLock<HashMap<PriceListKind, Instant>>>,
    cache_dir: Option<PathBuf>,
}

impl PriceListCache {
    /// Creates a cache whose entries live for `ttl`.
    pub fn new(ttl: Duration) -> Self {
        let locks = [PriceListKind::Prepaid, PriceListKind::Pasca]
            .into_iter()
            .map(|kind| (kind, Arc::new(Mutex::new(()))))
            .collect();

        let cache_dir = std::env::var("PRICE_LIST_CACHE_DIR")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from);

        let mut initial_entries = HashMap::new();
        for kind in [PriceListKind::Prepaid, PriceListKind::Pasca] {
            if let Some(list) = Self::read_disk_cache(kind, cache_dir.as_deref()) {
                tracing::info!(?kind, "preloaded price list snapshot from disk cache");
                initial_entries.insert(
                    kind,
                    Entry {
                        list,
                        // Expired initially so fresh fetch is attempted, but available for stale fallback
                        expires_at: Instant::now(),
                    },
                );
            }
        }

        Self {
            ttl,
            entries: Arc::new(RwLock::new(initial_entries)),
            locks: Arc::new(locks),
            cooldowns: Arc::new(RwLock::new(HashMap::new())),
            cache_dir,
        }
    }

    fn disk_path(kind: PriceListKind, dir: Option<&Path>) -> Option<PathBuf> {
        let dir = dir?;
        let filename = match kind {
            PriceListKind::Prepaid => "digiflazz_pricelist_prepaid.json",
            PriceListKind::Pasca => "digiflazz_pricelist_pasca.json",
        };
        Some(dir.join(filename))
    }

    fn read_disk_cache(kind: PriceListKind, dir: Option<&Path>) -> Option<PriceList> {
        let path = Self::disk_path(kind, dir)?;
        let content = std::fs::read_to_string(&path).ok()?;
        match kind {
            PriceListKind::Prepaid => {
                let items: Vec<PrepaidProduct> = serde_json::from_str(&content).ok()?;
                Some(PriceList::Prepaid(items))
            }
            PriceListKind::Pasca => {
                let items: Vec<PostpaidProduct> = serde_json::from_str(&content).ok()?;
                Some(PriceList::Pasca(items))
            }
        }
    }

    fn write_disk_cache(kind: PriceListKind, list: &PriceList, dir: Option<&Path>) {
        let Some(path) = Self::disk_path(kind, dir) else {
            return;
        };
        let serialized = match serde_json::to_string(list) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(?kind, error = %e, "failed to serialize price list for disk cache");
                return;
            }
        };

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let tmp_path = path.with_extension("tmp");
        if let Err(e) = std::fs::write(&tmp_path, serialized) {
            tracing::warn!(?kind, path = %tmp_path.display(), error = %e, "failed to write disk cache tmp file");
            return;
        }
        if let Err(e) = std::fs::rename(&tmp_path, &path) {
            tracing::warn!(?kind, path = %path.display(), error = %e, "failed to rename disk cache file");
        }
    }

    /// Returns a cached list, loading it through `load` when absent or expired.
    ///
    /// If `load` fails but a previously cached entry exists (in-memory or on disk),
    /// that entry is returned instead so transient upstream failures (e.g. rate limit
    /// RC 83) do not disrupt callers.
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

        // Check if rate-limit cooldown is active from a recent upstream rejection.
        let is_cooling_down = {
            let cooldowns = self.cooldowns.read().await;
            cooldowns
                .get(&kind)
                .map(|until| Instant::now() < *until)
                .unwrap_or(false)
        };

        if is_cooling_down {
            let entries = self.entries.read().await;
            if let Some(entry) = entries.get(&kind) {
                tracing::warn!(
                    ?kind,
                    "serving stale price list during upstream rate-limit cooldown"
                );
                return Ok(entry.list.clone());
            }
            if let Some(disk_list) = Self::read_disk_cache(kind, self.cache_dir.as_deref()) {
                tracing::warn!(
                    ?kind,
                    "serving disk cache price list during upstream rate-limit cooldown"
                );
                return Ok(disk_list);
            }
        }

        match load.await {
            Ok(list) => {
                Self::write_disk_cache(kind, &list, self.cache_dir.as_deref());
                self.cooldowns.write().await.remove(&kind);
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
            Err(error) => {
                // Set cooldown: 60s cooldown to prevent hammering Digiflazz after failure/rate limit
                self.cooldowns
                    .write()
                    .await
                    .insert(kind, Instant::now() + Duration::from_secs(60));

                let mut entries = self.entries.write().await;
                if let Some(entry) = entries.get_mut(&kind) {
                    tracing::warn!(
                        ?kind,
                        error = %error,
                        "serving stale price list after upstream failure (cooldown extended)"
                    );
                    // Extend expiration so callers don't bombard upstream while rate limited
                    entry.expires_at = Instant::now() + Duration::from_secs(120);
                    Ok(entry.list.clone())
                } else if let Some(disk_list) =
                    Self::read_disk_cache(kind, self.cache_dir.as_deref())
                {
                    tracing::warn!(
                        ?kind,
                        error = %error,
                        "loaded and serving stale price list from disk cache after upstream failure"
                    );
                    entries.insert(
                        kind,
                        Entry {
                            list: disk_list.clone(),
                            expires_at: Instant::now() + Duration::from_secs(120),
                        },
                    );
                    Ok(disk_list)
                } else {
                    Err(error)
                }
            }
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

    #[tokio::test]
    async fn serves_disk_cache_when_upstream_fails_on_fresh_cache() {
        let unique_dir = std::env::temp_dir().join(format!(
            "df_test_cache_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&unique_dir);
        let dummy = sample(PriceListKind::Prepaid);
        PriceListCache::write_disk_cache(PriceListKind::Prepaid, &dummy, Some(&unique_dir));

        let cache = PriceListCache {
            ttl: Duration::from_secs(0),
            entries: Arc::new(RwLock::new(HashMap::new())),
            locks: Arc::new(HashMap::from([(PriceListKind::Prepaid, Arc::new(Mutex::new(())))])),
            cooldowns: Arc::new(RwLock::new(HashMap::new())),
            cache_dir: Some(unique_dir.clone()),
        };

        let result = cache
            .get(PriceListKind::Prepaid, async {
                Err(digiflazz::Error::Rejected {
                    rc: "83".to_owned(),
                    message: "limit reached".to_owned(),
                })
            })
            .await;

        assert!(
            result.is_ok(),
            "should fall back to disk cache when upstream fails"
        );
        let _ = std::fs::remove_dir_all(&unique_dir);
    }
}
