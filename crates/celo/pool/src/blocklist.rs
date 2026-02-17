//! Fee currency blocklist management.
//!
//! Provides time-based blocklisting of fee currencies that malfunction during
//! block building (e.g., their debitGasFees/creditGasFees calls revert or
//! consume excessive gas).

use alloy_primitives::Address;
use parking_lot::RwLock;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

/// Default eviction timeout for blocked currencies (2 hours).
/// Matches Go's `EvictionTimeoutSeconds = 7200`.
pub(crate) const DEFAULT_EVICTION_TIMEOUT: Duration = Duration::from_secs(7200);

/// Manages a blocklist of fee currencies that have caused issues during block building.
///
/// Currencies are automatically unblocked after the eviction timeout.
/// Admin controls allow per-currency disable/enable of blocking.
///
/// This is a port of the Go `AddressBlocklist` from op-geth.
#[derive(Debug, Clone)]
pub struct CurrencyBlocklist {
    inner: Arc<RwLock<BlocklistInner>>,
}

#[derive(Debug)]
struct BlocklistInner {
    /// Map from fee currency address to when it was blocked.
    blocked: HashMap<Address, Instant>,
    /// Currencies where blocking is disabled (admin override).
    /// When blocking is disabled for a currency, it won't be blocked
    /// and filter_allowlist won't remove it.
    blocking_disabled: HashSet<Address>,
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
                blocking_disabled: HashSet::new(),
                eviction_timeout,
            })),
        }
    }

    /// Blocks a fee currency address. The currency will be automatically
    /// unblocked after the eviction timeout.
    pub fn block_currency(&self, currency: Address) {
        let mut inner = self.inner.write();
        inner.blocked.insert(currency, Instant::now());
    }

    /// Returns true if the given currency is currently blocked and
    /// blocking is enabled for it.
    pub fn is_blocked(&self, currency: &Address) -> bool {
        let inner = self.inner.read();

        // If blocking is disabled for this currency, it's never blocked
        if inner.blocking_disabled.contains(currency) {
            return false;
        }

        if let Some(blocked_at) = inner.blocked.get(currency) {
            blocked_at.elapsed() < inner.eviction_timeout
        } else {
            false
        }
    }

    /// Returns true if blocking is enabled for the given currency.
    /// Blocking is enabled by default; it can be disabled via admin API.
    pub fn blocking_enabled(&self, currency: &Address) -> bool {
        !self.inner.read().blocking_disabled.contains(currency)
    }

    /// Manually unblocks (removes) a fee currency from the blocklist.
    pub fn remove(&self, currency: &Address) {
        self.inner.write().blocked.remove(currency);
    }

    /// Evicts all currencies whose eviction timeout has passed.
    pub fn evict(&self) {
        let mut inner = self.inner.write();
        let timeout = inner.eviction_timeout;
        inner.blocked.retain(|_, blocked_at| blocked_at.elapsed() < timeout);
    }

    /// Disables blocking for the given currencies (admin override).
    pub fn disable_blocking(&self, currencies: &[Address]) {
        let mut inner = self.inner.write();
        for currency in currencies {
            inner.blocking_disabled.insert(*currency);
        }
    }

    /// Re-enables blocking for the given currencies.
    pub fn enable_blocking(&self, currencies: &[Address]) {
        let mut inner = self.inner.write();
        for currency in currencies {
            inner.blocking_disabled.remove(currency);
        }
    }

    /// Filters an allowlist by removing blocked currencies (unless blocking is disabled).
    /// Returns the set of currencies from the allowlist that are NOT blocked.
    pub fn filter_allowlist(&self, allowlist: &HashSet<Address>) -> HashSet<Address> {
        let inner = self.inner.read();
        let timeout = inner.eviction_timeout;

        allowlist
            .iter()
            .filter(|&addr| {
                // Keep the currency if blocking is disabled for it
                if inner.blocking_disabled.contains(addr) {
                    return true;
                }
                // Keep it if it's not blocked or its timeout expired
                match inner.blocked.get(addr) {
                    Some(blocked_at) => blocked_at.elapsed() >= timeout,
                    None => true,
                }
            })
            .copied()
            .collect()
    }

    /// Returns all blocked currencies with their remaining time.
    /// If `include_disabled` is true, includes currencies with blocking disabled.
    pub fn blocklist(&self, include_disabled: bool) -> HashMap<Address, Duration> {
        let inner = self.inner.read();
        let timeout = inner.eviction_timeout;

        inner
            .blocked
            .iter()
            .filter(|&(addr, blocked_at)| {
                // Only include non-expired entries
                if blocked_at.elapsed() >= timeout {
                    return false;
                }
                // Filter out disabled if requested
                if !include_disabled && inner.blocking_disabled.contains(addr) {
                    return false;
                }
                true
            })
            .map(|(addr, blocked_at)| {
                let remaining = timeout.saturating_sub(blocked_at.elapsed());
                (*addr, remaining)
            })
            .collect()
    }

    /// Returns the list of currencies with blocking disabled.
    pub fn disabled_currencies(&self) -> Vec<Address> {
        self.inner.read().blocking_disabled.iter().copied().collect()
    }

    /// Returns the list of currently blocked currencies (not expired).
    pub fn blocked_currencies(&self) -> Vec<Address> {
        let inner = self.inner.read();
        let timeout = inner.eviction_timeout;
        inner
            .blocked
            .iter()
            .filter(|(_, blocked_at)| blocked_at.elapsed() < timeout)
            .map(|(addr, _)| *addr)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr1() -> Address {
        Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1])
    }

    fn addr2() -> Address {
        Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2])
    }

    /// Port of TestBlocklistEviction
    #[test]
    fn test_blocklist_eviction() {
        let timeout = Duration::from_millis(50);
        let bl = CurrencyBlocklist::new(timeout);
        bl.block_currency(addr1());

        // Before timeout: blocked
        assert!(bl.is_blocked(&addr1()));

        // Filter allowlist removes it
        let allowlist = HashSet::from([addr1()]);
        assert_eq!(bl.filter_allowlist(&allowlist).len(), 0);

        // Wait for eviction
        std::thread::sleep(Duration::from_millis(60));

        // After timeout: not blocked
        assert!(!bl.is_blocked(&addr1()));

        // Evict expired entries
        bl.evict();

        // Currency is no longer in the blocklist at all
        assert!(!bl.is_blocked(&addr1()));

        // Filter allowlist doesn't remove it anymore
        assert_eq!(bl.filter_allowlist(&allowlist).len(), 1);
    }

    /// Port of TestBlocklistAddAfterEviction
    #[test]
    fn test_blocklist_add_after_eviction() {
        let timeout = Duration::from_millis(50);
        let bl = CurrencyBlocklist::new(timeout);
        bl.block_currency(addr1());

        // Wait for eviction of addr1
        std::thread::sleep(Duration::from_millis(60));
        bl.evict();

        // Add addr2 after eviction
        bl.block_currency(addr2());

        // addr2 should be blocked
        assert!(bl.is_blocked(&addr2()));

        // Wait for addr2's eviction
        std::thread::sleep(Duration::from_millis(60));
        assert!(!bl.is_blocked(&addr2()));
    }

    /// Port of TestBlocklistRemove
    #[test]
    fn test_blocklist_remove() {
        let timeout = Duration::from_secs(3600);
        let bl = CurrencyBlocklist::new(timeout);

        // Removing from empty blocklist doesn't panic
        bl.remove(&addr1());

        // Add two currencies
        bl.block_currency(addr1());
        bl.block_currency(addr2());

        // Remove one
        bl.remove(&addr1());

        assert!(!bl.is_blocked(&addr1()));
        assert!(bl.is_blocked(&addr2()));
    }

    /// Port of TestBlocklistAddAfterRemove
    #[test]
    fn test_blocklist_add_after_remove() {
        let timeout = Duration::from_secs(3600);
        let bl = CurrencyBlocklist::new(timeout);

        bl.block_currency(addr1());
        bl.remove(&addr1());
        assert!(!bl.is_blocked(&addr1()));

        // Add addr2
        bl.block_currency(addr2());
        assert!(bl.is_blocked(&addr2()));
    }

    /// Port of TestDisableEnableBlocking
    #[test]
    fn test_disable_enable_blocking() {
        let timeout = Duration::from_secs(3600);
        let bl = CurrencyBlocklist::new(timeout);

        bl.block_currency(addr1());
        bl.block_currency(addr2());

        assert!(bl.blocking_enabled(&addr1()));
        assert!(bl.blocking_enabled(&addr2()));

        let allowlist = HashSet::from([addr1(), addr2()]);

        // Both blocked: allowlist should be empty after filtering
        assert_eq!(bl.filter_allowlist(&allowlist).len(), 0);

        // Disable blocking for addr1
        bl.disable_blocking(&[addr1()]);
        assert!(!bl.blocking_enabled(&addr1()));
        let filtered = bl.filter_allowlist(&allowlist);
        assert!(filtered.contains(&addr1()));
        assert!(!filtered.contains(&addr2()));

        // Disable blocking for addr2
        bl.disable_blocking(&[addr2()]);
        assert!(!bl.blocking_enabled(&addr2()));
        assert_eq!(bl.filter_allowlist(&allowlist), allowlist);

        // Re-enable blocking for both
        bl.enable_blocking(&[addr1(), addr2()]);
        assert!(bl.blocking_enabled(&addr1()));
        assert!(bl.blocking_enabled(&addr2()));
        assert_eq!(bl.filter_allowlist(&allowlist).len(), 0);
    }

    /// Port of TestBlocklistRetrieval
    #[test]
    fn test_blocklist_retrieval() {
        let timeout = Duration::from_secs(3600);
        let bl = CurrencyBlocklist::new(timeout);

        bl.block_currency(addr1());
        bl.block_currency(addr2());

        // Both should be in the blocklist
        let all = bl.blocklist(true);
        assert_eq!(all.len(), 2);
        assert!(all.contains_key(&addr1()));
        assert!(all.contains_key(&addr2()));

        let active = bl.blocklist(false);
        assert_eq!(active.len(), 2);

        // Disable blocking for addr1
        bl.disable_blocking(&[addr1()]);

        // include_disabled=true still shows both
        assert_eq!(bl.blocklist(true).len(), 2);

        // include_disabled=false only shows addr2
        let active = bl.blocklist(false);
        assert_eq!(active.len(), 1);
        assert!(active.contains_key(&addr2()));
    }

    /// Port of TestDisabledCurrenciesRetrieval
    #[test]
    fn test_disabled_currencies_retrieval() {
        let bl = CurrencyBlocklist::new(Duration::from_secs(3600));

        assert!(bl.disabled_currencies().is_empty());

        bl.disable_blocking(&[addr1(), addr2()]);
        let disabled = bl.disabled_currencies();
        assert_eq!(disabled.len(), 2);
        assert!(disabled.contains(&addr1()));
        assert!(disabled.contains(&addr2()));

        bl.enable_blocking(&[addr1()]);
        let disabled = bl.disabled_currencies();
        assert_eq!(disabled.len(), 1);
        assert!(disabled.contains(&addr2()));
    }

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

        blocklist.remove(&currency);
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
