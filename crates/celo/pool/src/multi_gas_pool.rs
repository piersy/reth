//! Multi-gas pool system for Celo fee currencies.
//!
//! During block building, each fee currency gets a separate gas budget to prevent
//! one currency from consuming the entire block's gas limit.

use alloy_primitives::Address;
use std::collections::{HashMap, HashSet};

/// Type alias for fee currency address. `None` represents native CELO.
pub type FeeCurrency = Option<Address>;

/// Manages separate gas pools for different fee currencies during block building.
///
/// Each registered fee currency gets a fraction of the total block gas limit.
/// The default pool (for native CELO) always gets the full gas limit.
/// Unregistered currencies fall back to the default pool.
#[derive(Debug, Clone)]
pub struct MultiGasPool {
    /// The total block gas limit.
    block_gas_limit: u64,
    /// Default (native CELO) gas pool remaining gas.
    default_pool: u64,
    /// Per-currency gas pools: maps fee currency address to remaining gas.
    currency_pools: HashMap<Address, u64>,
    /// Set of registered/allowed fee currencies.
    allowlist: HashSet<Address>,
    /// Default fraction of block gas limit for registered currencies without explicit limits.
    default_limit: f64,
    /// Per-currency limit fractions.
    limits: HashMap<Address, f64>,
}

impl MultiGasPool {
    /// Creates a new multi-gas pool.
    ///
    /// - `block_gas_limit`: Total gas limit for the block.
    /// - `allowlist`: Set of registered fee currency addresses.
    /// - `default_limit`: Default fraction of block gas for each registered currency.
    /// - `limits`: Per-currency fraction overrides.
    pub fn new(
        block_gas_limit: u64,
        allowlist: HashSet<Address>,
        default_limit: f64,
        limits: HashMap<Address, f64>,
    ) -> Self {
        Self {
            block_gas_limit,
            default_pool: block_gas_limit,
            currency_pools: HashMap::new(),
            allowlist,
            default_limit,
            limits,
        }
    }

    /// Returns the gas pool for the given fee currency, and whether it's a
    /// dedicated multi-pool (true) or the default pool (false).
    ///
    /// - `None` (native CELO) always uses the default pool.
    /// - Registered currencies get a dedicated pool.
    /// - Unregistered currencies fall back to the default pool.
    pub fn pool_for(&mut self, fee_currency: FeeCurrency) -> (u64, bool) {
        match fee_currency {
            None => (self.default_pool, false),
            Some(addr) => {
                if !self.allowlist.contains(&addr) {
                    // Unregistered currency falls back to default pool
                    return (self.default_pool, false);
                }
                // Registered currency: initialize pool if needed
                let limit = self.limits.get(&addr).copied().unwrap_or(self.default_limit);
                let gas =
                    *self.currency_pools.entry(addr).or_insert_with(|| {
                        (self.block_gas_limit as f64 * limit) as u64
                    });
                (gas, true)
            }
        }
    }

    /// Consumes gas from the pool for the given fee currency.
    /// Returns `true` if there was enough gas, `false` otherwise.
    pub fn use_gas(&mut self, fee_currency: FeeCurrency, gas: u64) -> bool {
        let (remaining, is_multi) = self.pool_for(fee_currency);
        if remaining < gas {
            return false;
        }
        if is_multi {
            if let Some(addr) = fee_currency {
                *self.currency_pools.get_mut(&addr).unwrap() -= gas;
            }
        } else {
            self.default_pool -= gas;
        }
        true
    }

    /// Returns the remaining gas in the default (native CELO) pool.
    pub const fn default_gas(&self) -> u64 {
        self.default_pool
    }

    /// Returns the block gas limit.
    pub const fn block_gas_limit(&self) -> u64 {
        self.block_gas_limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cusd() -> Address {
        "0x765DE816845861e75A25fCA122bb6898B8B1282a".parse().unwrap()
    }

    fn ceur() -> Address {
        "0xD8763CBa276a3738E6DE85b4b3bF5FDed6D6cA73".parse().unwrap()
    }

    /// Port of TestMultiCurrencyGasPool from Go:
    /// "Empty allowlist, empty mapping, CELO uses default pool"
    #[test]
    fn test_empty_allowlist_celo_uses_default_pool() {
        let block_gas_limit = 1000u64;
        let sub_gas = 100u64;

        let mut mgp = MultiGasPool::new(
            block_gas_limit,
            HashSet::new(),
            0.9,
            HashMap::new(),
        );

        let (_, is_multi) = mgp.pool_for(None);
        assert!(!is_multi);

        mgp.use_gas(None, sub_gas);
        let (remaining, _) = mgp.pool_for(None);
        assert_eq!(remaining, 900);
    }

    /// "Non-empty allowlist, non-empty mapping, CELO uses default pool"
    #[test]
    fn test_nonempty_allowlist_celo_uses_default_pool() {
        let block_gas_limit = 1000u64;
        let sub_gas = 100u64;

        let mut limits = HashMap::new();
        limits.insert(cusd(), 0.5);

        let mut mgp = MultiGasPool::new(
            block_gas_limit,
            HashSet::from([cusd()]),
            0.9,
            limits,
        );

        let (_, is_multi) = mgp.pool_for(None);
        assert!(!is_multi);

        mgp.use_gas(None, sub_gas);
        let (remaining, _) = mgp.pool_for(None);
        assert_eq!(remaining, 900);
    }

    /// "Empty allowlist, empty mapping, non-registered currency fallbacks to the default pool"
    #[test]
    fn test_empty_allowlist_unregistered_fallback() {
        let block_gas_limit = 1000u64;
        let sub_gas = 100u64;

        let mut mgp = MultiGasPool::new(
            block_gas_limit,
            HashSet::new(),
            0.9,
            HashMap::new(),
        );

        let (_, is_multi) = mgp.pool_for(Some(cusd()));
        assert!(!is_multi);

        mgp.use_gas(Some(cusd()), sub_gas);
        let (remaining, _) = mgp.pool_for(Some(cusd()));
        assert_eq!(remaining, 900);
    }

    /// "Non-empty allowlist, non-empty mapping, non-registered currency uses default pool"
    #[test]
    fn test_nonempty_allowlist_unregistered_uses_default() {
        let block_gas_limit = 1000u64;
        let sub_gas = 100u64;

        let mut limits = HashMap::new();
        limits.insert(cusd(), 0.5);

        let mut mgp = MultiGasPool::new(
            block_gas_limit,
            HashSet::from([cusd()]),
            0.9,
            limits,
        );

        // cEUR is NOT in the allowlist
        let (_, is_multi) = mgp.pool_for(Some(ceur()));
        assert!(!is_multi);

        mgp.use_gas(Some(ceur()), sub_gas);
        let (remaining, _) = mgp.pool_for(Some(ceur()));
        assert_eq!(remaining, 900);
    }

    /// "Non-empty allowlist, empty mapping, registered currency uses default limit"
    #[test]
    fn test_registered_currency_uses_default_limit() {
        let block_gas_limit = 1000u64;
        let sub_gas = 100u64;

        let mut mgp = MultiGasPool::new(
            block_gas_limit,
            HashSet::from([cusd()]),
            0.9,
            HashMap::new(),
        );

        let (_, is_multi) = mgp.pool_for(Some(cusd()));
        assert!(is_multi);

        // Default pool should be untouched
        let (default_gas, _) = mgp.pool_for(None);
        assert_eq!(default_gas, block_gas_limit);

        mgp.use_gas(Some(cusd()), sub_gas);
        let (remaining, _) = mgp.pool_for(Some(cusd()));
        // 1000 * 0.9 - 100 = 800
        assert_eq!(remaining, 800);
    }

    /// "Non-empty allowlist, non-empty mapping, configured registered currency uses configured limits"
    #[test]
    fn test_registered_currency_uses_configured_limit() {
        let block_gas_limit = 1000u64;
        let sub_gas = 100u64;

        let mut limits = HashMap::new();
        limits.insert(cusd(), 0.5);

        let mut mgp = MultiGasPool::new(
            block_gas_limit,
            HashSet::from([cusd()]),
            0.9,
            limits,
        );

        let (_, is_multi) = mgp.pool_for(Some(cusd()));
        assert!(is_multi);

        // Default pool should be untouched
        let (default_gas, _) = mgp.pool_for(None);
        assert_eq!(default_gas, block_gas_limit);

        mgp.use_gas(Some(cusd()), sub_gas);
        let (remaining, _) = mgp.pool_for(Some(cusd()));
        // 1000 * 0.5 - 100 = 400
        assert_eq!(remaining, 400);
    }

    /// "Non-empty allowlist, non-empty mapping, unconfigured registered currency uses default limit"
    #[test]
    fn test_unconfigured_registered_uses_default_limit() {
        let block_gas_limit = 1000u64;
        let sub_gas = 100u64;

        let mut limits = HashMap::new();
        limits.insert(cusd(), 0.5);

        let mut mgp = MultiGasPool::new(
            block_gas_limit,
            HashSet::from([cusd(), ceur()]),
            0.9,
            limits,
        );

        // cEUR is registered but has no explicit limit, uses default_limit=0.9
        let (_, is_multi) = mgp.pool_for(Some(ceur()));
        assert!(is_multi);

        mgp.use_gas(Some(ceur()), sub_gas);
        let (remaining, _) = mgp.pool_for(Some(ceur()));
        // 1000 * 0.9 - 100 = 800
        assert_eq!(remaining, 800);
    }

    #[test]
    fn test_independent_pools() {
        let mut mgp = MultiGasPool::new(
            30_000_000,
            HashSet::from([cusd(), ceur()]),
            0.5,
            HashMap::new(),
        );

        mgp.use_gas(Some(cusd()), 10_000_000);
        // cEUR should still have its full allocation
        let (remaining, _) = mgp.pool_for(Some(ceur()));
        assert_eq!(remaining, 15_000_000);
    }

    #[test]
    fn test_use_gas_exceeds_limit() {
        let mut mgp = MultiGasPool::new(
            100,
            HashSet::from([cusd()]),
            0.5,
            HashMap::new(),
        );

        // Fee currency gets 50 gas
        assert!(!mgp.use_gas(Some(cusd()), 51));
        // Gas should not have been consumed
        let (remaining, _) = mgp.pool_for(Some(cusd()));
        assert_eq!(remaining, 50);
    }
}
