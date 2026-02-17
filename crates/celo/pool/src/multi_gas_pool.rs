//! Multi-gas pool system for Celo fee currencies.
//!
//! During block building, each fee currency gets a separate gas budget to prevent
//! one currency from consuming the entire block's gas limit.

use alloy_primitives::Address;
use std::collections::HashMap;

/// Manages separate gas pools for different fee currencies during block building.
///
/// Each fee currency gets a fraction of the total block gas limit. The default
/// pool (for native CELO) gets the full gas limit. This prevents a single fee
/// currency's transactions from monopolizing block space.
#[derive(Debug, Clone)]
pub struct MultiGasPool {
    /// The total block gas limit.
    block_gas_limit: u64,
    /// Per-currency gas pools: maps fee currency address to remaining gas.
    pools: HashMap<Option<Address>, u64>,
    /// Default fraction of block gas limit for each fee currency pool.
    default_fraction: f64,
}

impl MultiGasPool {
    /// Creates a new multi-gas pool with the given block gas limit.
    pub fn new(block_gas_limit: u64, default_fraction: f64) -> Self {
        let mut pools = HashMap::new();
        // Native CELO (None) gets the full block gas limit
        pools.insert(None, block_gas_limit);

        Self { block_gas_limit, pools, default_fraction }
    }

    /// Returns the remaining gas for the given fee currency.
    /// If the currency hasn't been seen before, initializes its pool.
    pub fn gas_remaining(&mut self, fee_currency: Option<Address>) -> u64 {
        *self.pools.entry(fee_currency).or_insert_with(|| {
            // New fee currency gets a fraction of the block gas limit
            (self.block_gas_limit as f64 * self.default_fraction) as u64
        })
    }

    /// Consumes gas from the pool for the given fee currency.
    /// Returns `true` if there was enough gas, `false` otherwise.
    pub fn use_gas(&mut self, fee_currency: Option<Address>, gas: u64) -> bool {
        let remaining = self.gas_remaining(fee_currency);
        if remaining >= gas {
            *self.pools.get_mut(&fee_currency).unwrap() -= gas;
            true
        } else {
            false
        }
    }

    /// Sets a specific gas limit for a fee currency pool.
    pub fn set_pool_limit(&mut self, fee_currency: Option<Address>, limit: u64) {
        self.pools.insert(fee_currency, limit);
    }

    /// Returns the block gas limit.
    pub const fn block_gas_limit(&self) -> u64 {
        self.block_gas_limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_gets_full_limit() {
        let mut pool = MultiGasPool::new(30_000_000, 0.9);
        assert_eq!(pool.gas_remaining(None), 30_000_000);
    }

    #[test]
    fn test_fee_currency_gets_fraction() {
        let mut pool = MultiGasPool::new(30_000_000, 0.9);
        let currency = Some(Address::random());
        assert_eq!(pool.gas_remaining(currency), 27_000_000);
    }

    #[test]
    fn test_use_gas() {
        let mut pool = MultiGasPool::new(30_000_000, 0.9);
        assert!(pool.use_gas(None, 21_000));
        assert_eq!(pool.gas_remaining(None), 30_000_000 - 21_000);
    }

    #[test]
    fn test_use_gas_exceeds_limit() {
        let mut pool = MultiGasPool::new(100, 0.5);
        let currency = Some(Address::random());
        // Fee currency gets 50 gas
        assert!(!pool.use_gas(currency, 51));
        // Gas should not have been consumed
        assert_eq!(pool.gas_remaining(currency), 50);
    }

    #[test]
    fn test_independent_pools() {
        let mut pool = MultiGasPool::new(30_000_000, 0.5);
        let currency_a = Some(Address::random());
        let currency_b = Some(Address::random());

        pool.use_gas(currency_a, 10_000_000);
        // currency_b should still have its full allocation
        assert_eq!(pool.gas_remaining(currency_b), 15_000_000);
    }
}
