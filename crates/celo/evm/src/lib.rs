//! Celo EVM configuration for reth.
//!
//! This crate provides the [`CeloEvmConfig`] which integrates celo-revm's Celo-specific
//! EVM modifications with reth's execution framework. The EVM-level changes (CIP-64 fee
//! currencies, transfer precompile, etc.) are handled by celo-revm; this crate provides
//! the reth integration layer.
//!
//! # Architecture
//!
//! Celo extends the OP Stack EVM with:
//! - **CIP-64 fee currencies**: Transactions can pay gas fees in ERC-20 tokens
//! - **Transfer precompile**: CELO functions as both native currency and ERC-20
//! - **Fee currency context**: Exchange rates cached per block
//! - **Legacy chain ID exceptions**: Historical compatibility for specific transactions

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

use reth_celo_chainspec::CeloChainSpec;

mod config;
pub use config::celo_spec;

/// Celo-specific EVM configuration.
///
/// This is currently an alias for the Ethereum EVM configuration parameterized
/// with [`CeloChainSpec`]. Celo-specific EVM modifications (CIP-64 fee currencies,
/// transfer precompile, etc.) are handled by celo-revm at the revm level.
///
/// In the future, this may be replaced with a custom type that integrates
/// celo-revm's handler modifications directly.
pub type CeloEvmConfig = reth_evm_ethereum::EthEvmConfig<CeloChainSpec>;

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_consensus::Header;
    use reth_celo_chainspec::CELO_DEV;
    use reth_chainspec::EthChainSpec;
    use reth_evm::ConfigureEvm;

    #[test]
    fn test_celo_evm_config_creation() {
        let config = CeloEvmConfig::new(CELO_DEV.clone());
        assert_eq!(config.chain_spec().chain().id(), 11142220);
    }

    #[test]
    fn test_celo_evm_env() {
        let config = CeloEvmConfig::new(CELO_DEV.clone());
        let header = Header::default();
        let env = config.evm_env(&header).unwrap();
        assert_eq!(env.cfg_env.chain_id, 11142220);
    }
}
