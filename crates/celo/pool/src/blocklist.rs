//! Fee currency blocklist management.
//!
//! Provides time-based blocklisting of fee currencies that malfunction during
//! block building (e.g., their debitGasFees/creditGasFees calls revert or
//! consume excessive gas).

use alloy_primitives::Address;
use parking_lot::RwLock;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

/// Default eviction timeout for blocked currencies (2 hours).
const DEFAULT_EVICTION_TIMEOUT: Duration = Duration::from_secs(7200);

/// Manages a blocklist of fee currencies that have caused issues during block building.
///
/// Currencies are automatically unblocked after the eviction timeout.
/// Admin controls allow manual blocking/unblocking.
#[derive(Debug, Clone)]
pub struct CurrencyBlocklist {
    inner: Arc<RwLock<BlocklistInner>>,
}

#[derive(Debug)]
struct BlocklistInner {
    /// Map from fee currency address to when it was blocked.
    blocked: HashMap<Address, Instant>,
    /// Currencies that are exempt from blocking (admin override).
    exempt: HashMap<Address, bool>,
    /// Eviction timeout for automatically unblocking currencies.
    eviction_timeout: Duration,
}

impl Default for CurrencyBlocklist {
    fn default() -> Self {
        Self::new(DEFAULT_EVICTION_TIMEOUT)
    }
}

impl CurrencyBlocklist {
    /// Creates a new blocklist with the given eviction timeout.
    pub fn new(eviction_timeout: Duration) -> Self {
        Self {
            inner: Arc::new(RwLock::new(BlocklistInner {
                blocked: HashMap::new(),
                exempt: HashMap::new(),
                eviction_timeout,
            })),
        }
    }

    /// Blocks a fee currency address. The currency will be automatically
    /// unblocked after the eviction timeout.
    pub fn block_currency(&self, currency: Address) {
        let mut inner = self.inner.write();
        // Only block if not exempt
        if !inner.exempt.get(&currency).copied().unwrap_or(false) {
            inner.blocked.insert(currency, Instant::now());
        }
    }

    /// Returns true if the given currency is currently blocked.
    pub fn is_blocked(&self, currency: &Address) -> bool {
        let mut inner = self.inner.write();

        if let Some(blocked_at) = inner.blocked.get(currency) {
            if blocked_at.elapsed() >= inner.eviction_timeout {
                // Eviction timeout reached, automatically unblock
                inner.blocked.remove(currency);
                false
            } else {
                true
            }
        } else {
            false
        }
    }

    /// Manually unblocks a fee currency.
    pub fn unblock_currency(&self, currency: &Address) {
        self.inner.write().blocked.remove(currency);
    }

    /// Disables blocking for a currency (admin override).
    /// The currency will not be blocked even if it causes issues.
    pub fn disable_blocking(&self, currency: Address) {
        let mut inner = self.inner.write();
        inner.exempt.insert(currency, true);
        // Also unblock if currently blocked
        inner.blocked.remove(&currency);
    }

    /// Re-enables blocking for a currency (reverses admin override).
    pub fn enable_blocking(&self, currency: &Address) {
        self.inner.write().exempt.remove(currency);
    }

    /// Returns the list of currently blocked currencies.
    pub fn blocked_currencies(&self) -> Vec<Address> {
        let inner = self.inner.read();
        inner.blocked.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_and_check() {
        let blocklist = CurrencyBlocklist::new(Duration::from_secs(3600));
        let currency = Address::random();

        assert!(!blocklist.is_blocked(&currency));
        blocklist.block_currency(currency);
        assert!(blocklist.is_blocked(&currency));
    }

    #[test]
    fn test_manual_unblock() {
        let blocklist = CurrencyBlocklist::new(Duration::from_secs(3600));
        let currency = Address::random();

        blocklist.block_currency(currency);
        assert!(blocklist.is_blocked(&currency));

        blocklist.unblock_currency(&currency);
        assert!(!blocklist.is_blocked(&currency));
    }

    #[test]
    fn test_exempt_currency() {
        let blocklist = CurrencyBlocklist::new(Duration::from_secs(3600));
        let currency = Address::random();

        blocklist.disable_blocking(currency);
        blocklist.block_currency(currency);
        assert!(!blocklist.is_blocked(&currency));
    }

    #[test]
    fn test_eviction_timeout() {
        let blocklist = CurrencyBlocklist::new(Duration::from_millis(1));
        let currency = Address::random();

        blocklist.block_currency(currency);
        // Sleep briefly to exceed the 1ms timeout
        std::thread::sleep(Duration::from_millis(10));
        assert!(!blocklist.is_blocked(&currency));
    }
}
