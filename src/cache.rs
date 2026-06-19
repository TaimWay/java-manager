//! TTL-based cache for Java search results.

use crate::{JavaError, JavaInfo};
use std::time::{Duration, Instant};

const DEFAULT_TTL: Duration = Duration::from_secs(10);

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
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// Return cached results if they are still fresh, otherwise
    /// run `fetcher` and cache the new results.
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
    pub fn clear(&mut self) {
        self.results.clear();
        self.cached_at = None;
    }

    fn is_fresh(&self) -> bool {
        self.cached_at.is_some_and(|t| t.elapsed() < self.ttl)
    }
}
