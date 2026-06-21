//! TTL-based cache for Java search results.

use crate::{JavaError, JavaInfo};
use std::time::{Duration, Instant};

const DEFAULT_TTL: Duration = Duration::from_secs(300);

/// A simple TTL-based cache for Java search results.
///
/// Java installations rarely change during a session, so caching
/// avoids repeated full-disk scans. Call [`get_or_refresh`] to
/// retrieve cached results or run a fetcher when the TTL expires.
///
/// [`get_or_refresh`]: JavaCache::get_or_refresh
#[derive(Debug)]
pub struct JavaCache {
    results: Vec<JavaInfo>,
    cached_at: Option<Instant>,
    ttl: Duration,
}

impl Default for JavaCache {
    fn default() -> Self {
        Self::new(DEFAULT_TTL)
    }
}

impl JavaCache {
    /// Create a new cache with a given TTL.
    ///
    /// After `ttl` elapses, the next call to [`get_or_refresh`]
    /// will run the fetcher again.
    ///
    /// [`get_or_refresh`]: JavaCache::get_or_refresh
    pub fn new(ttl: Duration) -> Self {
        Self {
            results: Vec::new(),
            cached_at: None,
            ttl,
        }
    }

    /// Set a custom TTL (builder-style).
    ///
    /// # Examples
    ///
    /// ```
    /// use java_manager::JavaCache;
    /// use std::time::Duration;
    ///
    /// let cache = JavaCache::new(Duration::from_secs(60)).ttl(Duration::from_secs(30));
    /// ```
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// Return cached results if they are still fresh, otherwise
    /// run `fetcher` and cache the new results.
    ///
    /// # Errors
    ///
    /// Propagates any error returned by `fetcher`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use java_manager::{JavaCache, full_search};
    /// use std::time::Duration;
    ///
    /// let mut cache = JavaCache::new(Duration::from_secs(300));
    /// let javas = cache.get_or_refresh(|| full_search())?;
    /// println!("Found {} Java(s)", javas.len());
    /// # Ok::<_, java_manager::JavaError>(())
    /// ```
    pub fn get_or_refresh<F>(&mut self, fetcher: F) -> Result<&[JavaInfo], JavaError>
    where
        F: Fn() -> Result<Vec<JavaInfo>, JavaError>,
    {
        if self.is_fresh() {
            log::debug!(
                "JavaCache: returning {} cached result(s)",
                self.results.len()
            );
            return Ok(&self.results);
        }

        log::debug!("JavaCache: cache expired, fetching...");
        self.results = fetcher()?;
        self.cached_at = Some(Instant::now());
        Ok(&self.results)
    }

    /// Force a refresh, ignoring the TTL.
    ///
    /// # Errors
    ///
    /// Propagates any error returned by `fetcher`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use java_manager::{JavaCache, full_search};
    /// use std::time::Duration;
    ///
    /// let mut cache = JavaCache::new(Duration::from_secs(300));
    /// let javas = cache.force_refresh(|| full_search())?;
    /// println!("Found {} Java(s)", javas.len());
    /// # Ok::<_, java_manager::JavaError>(())
    /// ```
    pub fn force_refresh<F>(&mut self, fetcher: F) -> Result<&[JavaInfo], JavaError>
    where
        F: Fn() -> Result<Vec<JavaInfo>, JavaError>,
    {
        log::debug!("JavaCache: force refresh");
        self.results = fetcher()?;
        self.cached_at = Some(Instant::now());
        Ok(&self.results)
    }

    /// Clear the cache.
    ///
    /// After calling `clear()`, the next [`get_or_refresh`](JavaCache::get_or_refresh)
    /// call will always run the fetcher regardless of the TTL.
    pub fn clear(&mut self) {
        self.results.clear();
        self.cached_at = None;
    }

    fn is_fresh(&self) -> bool {
        self.cached_at.is_some_and(|t| t.elapsed() < self.ttl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_fetcher(count: usize) -> impl Fn() -> Result<Vec<JavaInfo>, JavaError> {
        move || {
            let mut results = Vec::new();
            for i in 0..count {
                results.push(JavaInfo {
                    name: format!("Java {}", i),
                    version: format!("{}.0.0", 8 + i),
                    ..Default::default()
                });
            }
            Ok(results)
        }
    }

    #[test]
    fn test_cache_initial_miss() {
        let mut cache = JavaCache::new(Duration::from_secs(60));
        let results = cache.get_or_refresh(dummy_fetcher(2)).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_cache_hit_within_ttl() {
        let mut cache = JavaCache::new(Duration::from_secs(60));
        let _ = cache.get_or_refresh(dummy_fetcher(1)).unwrap();
        let results = cache.get_or_refresh(dummy_fetcher(999)).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_force_refresh() {
        let mut cache = JavaCache::new(Duration::from_secs(60));
        let _ = cache.get_or_refresh(dummy_fetcher(1)).unwrap();
        let results = cache.force_refresh(dummy_fetcher(3)).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_clear() {
        let mut cache = JavaCache::new(Duration::from_secs(60));
        let _ = cache.get_or_refresh(dummy_fetcher(1)).unwrap();
        cache.clear();
        assert!(cache.cached_at.is_none());
    }

    #[test]
    fn test_default_ttl() {
        let cache = JavaCache::default();
        assert_eq!(cache.ttl, DEFAULT_TTL);
    }

    #[test]
    fn test_custom_ttl_builder() {
        let cache = JavaCache::new(Duration::from_secs(10)).ttl(Duration::from_secs(30));
        assert_eq!(cache.ttl, Duration::from_secs(30));
    }

    #[test]
    fn test_cache_fetcher_error() {
        let mut cache = JavaCache::new(Duration::from_secs(60));
        let result: Result<&[JavaInfo], JavaError> =
            cache.get_or_refresh(|| Err(JavaError::Other("fetch failed".into())));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("fetch failed"));
    }

    #[test]
    fn test_cache_ttl_zero() {
        let mut cache = JavaCache::new(Duration::ZERO);
        let r1 = cache.get_or_refresh(dummy_fetcher(1)).unwrap();
        assert_eq!(r1.len(), 1);
        // TTL is zero → second call should re-fetch
        let r2 = cache.get_or_refresh(dummy_fetcher(2)).unwrap();
        assert_eq!(r2.len(), 2);
    }
}
