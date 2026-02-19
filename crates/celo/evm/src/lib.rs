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
//!
//! # EVM Factory
//!
//! The [`CeloEvmFactory`] (re-exported from `alloy-celo-evm`) produces Celo-specific EVM
//! instances that include:
//! - All OP Stack precompiles
//! - The Celo transfer precompile (for CELO token duality)
//! - The Celo handler with CIP-64 fee currency processing
//!
//! The factory uses [`PrecompilesMap`](alloy_evm::precompiles::PrecompilesMap) as its
//! precompile type, making it compatible with reth's `ConfigureEvm` requirements.
//!
//! # Integration Status
//!
//! Currently [`CeloEvmConfig`] uses `EthEvmConfig` as a temporary base. Full integration
//! with [`CeloEvmFactory`] requires `CeloPrimitives` (with `CeloTxEnvelope` as the
//! transaction type) to be implemented, so that `ConfigureEvm` can properly handle
//! CIP-64 transactions through the execution pipeline.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

use reth_celo_chainspec::CeloChainSpec;

// Re-export celo-revm and alloy-celo-evm types for reth integration
pub use alloy_celo_evm::CeloEvmFactory;
pub use celo_revm::{CeloEvm, CeloPrecompiles, celo_precompiles_map};

mod config;
pub use config::celo_spec;

/// Celo-specific EVM configuration.
///
/// Currently uses `EthEvmConfig` with the [`CeloChainSpec`] as a temporary solution.
/// This provides correct hardfork-based spec selection but uses the standard Ethereum
/// EVM factory (without Celo-specific precompiles or handler modifications).
///
/// To use the full Celo EVM with transfer precompile and CIP-64 support, use
/// [`CeloEvmFactory`] directly. Full `ConfigureEvm` integration is pending
/// implementation of `CeloPrimitives`.
pub type CeloEvmConfig = reth_evm_ethereum::EthEvmConfig<CeloChainSpec>;

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_consensus::Header;
    use alloy_evm::{Evm, EvmEnv, EvmFactory};
    use op_revm::OpSpecId;
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

    #[test]
    fn test_celo_evm_factory_creates_evm_with_precompiles_map() {
        let factory = CeloEvmFactory::default();
        let env = EvmEnv::default();
        let evm = factory.create_evm(revm::database::EmptyDB::default(), env);

        // Verify the EVM was created successfully and has the transfer precompile
        let transfer_address = celo_revm::precompiles::TRANSFER_ADDRESS;
        let precompiles = evm.precompiles();
        assert!(
            precompiles.get(&transfer_address).is_some(),
            "Transfer precompile should be registered in the PrecompilesMap"
        );
    }

    #[test]
    fn test_celo_precompiles_map_includes_op_precompiles() {
        let map = celo_precompiles_map(OpSpecId::ISTHMUS);
        // OP precompiles should be included (e.g., ecrecover at 0x1)
        let ecrecover = revm::primitives::Address::with_last_byte(1);
        assert!(
            map.get(&ecrecover).is_some(),
            "Standard precompiles should be included"
        );
    }
}
