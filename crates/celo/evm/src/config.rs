//! Celo-specific EVM spec configuration.
//!
//! Maps Celo hardfork activations to revm SpecId values.

use reth_celo_chainspec::CeloChainSpec;
use revm::primitives::hardfork::SpecId;

/// Returns the revm [`SpecId`] for the given Celo chain spec at the given timestamp
/// and block number.
///
/// Celo is a post-merge chain, so all blocks are post-Paris. The spec is determined
/// primarily by timestamp-based hardfork activations.
pub fn celo_spec(chain_spec: &CeloChainSpec, timestamp: u64, block_number: u64) -> SpecId {
    // Use the Ethereum hardfork detection from reth
    reth_evm_ethereum::revm_spec_by_timestamp_and_block_number(
        chain_spec,
        timestamp,
        block_number,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use reth_celo_chainspec::CELO_DEV;

    #[test]
    fn test_celo_spec_dev() {
        // Dev chain should be at least Cancun
        let spec = celo_spec(&CELO_DEV, 0, 0);
        assert!(spec >= SpecId::CANCUN);
    }
}
