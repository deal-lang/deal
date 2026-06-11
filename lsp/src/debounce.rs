//! Per-URI debouncer for did_change events (D-43).
//!
//! Each call to `schedule(uri, delay, action)` cancels the prior pending
//! task for that URI (if any) and spawns a new tokio task that sleeps for
//! `delay` and then runs `action`. The result: a burst of did_change
//! events within the debounce window collapses into exactly one re-parse
//! (the one corresponding to the last keystroke).
//!
//! Plan-frozen window: 300 ms (D-43 / RESEARCH §6).

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tokio::task::JoinHandle;
use tower_lsp::lsp_types::Url;

/// Per-URI debouncer.
///
/// Cloneable via `Arc<Debouncer>` — share a single instance across the
/// Backend struct.
pub struct Debouncer {
    schedules: DashMap<Url, JoinHandle<()>>,
}

impl Debouncer {
    pub fn new() -> Self {
        Self {
            schedules: DashMap::new(),
        }
    }

    /// Schedule `action` to run after `delay`, cancelling any previously
    /// scheduled action for the same URI.
    ///
    /// The action is a boxed future to keep the API monomorphic; callers
    /// supply `Box::pin(async move { ... })`.
    pub fn schedule<F>(self: &Arc<Self>, uri: Url, delay: Duration, action: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        // Cancel prior scheduled task for this URI (if any).
        if let Some((_, prior)) = self.schedules.remove(&uri) {
            prior.abort();
        }

        let handle = tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            action.await;
        });
        self.schedules.insert(uri, handle);
    }
}

impl Default for Debouncer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn rapid_schedules_collapse_to_last() {
        let debouncer = Arc::new(Debouncer::new());
        let counter = Arc::new(AtomicUsize::new(0));
        let uri = Url::parse("file:///x.deal").unwrap();

        // Fire 5 schedules in quick succession; each cancels the prior.
        for _ in 0..5 {
            let c = counter.clone();
            debouncer.schedule(uri.clone(), Duration::from_millis(50), async move {
                c.fetch_add(1, Ordering::SeqCst);
            });
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        // Wait past the debounce window.
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "expected exactly one action to fire (debounce coalescing)"
        );
    }

    #[tokio::test]
    async fn distinct_uris_do_not_cancel_each_other() {
        let debouncer = Arc::new(Debouncer::new());
        let counter = Arc::new(AtomicUsize::new(0));
        let uri_a = Url::parse("file:///a.deal").unwrap();
        let uri_b = Url::parse("file:///b.deal").unwrap();

        let c1 = counter.clone();
        debouncer.schedule(uri_a, Duration::from_millis(20), async move {
            c1.fetch_add(1, Ordering::SeqCst);
        });
        let c2 = counter.clone();
        debouncer.schedule(uri_b, Duration::from_millis(20), async move {
            c2.fetch_add(1, Ordering::SeqCst);
        });

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
