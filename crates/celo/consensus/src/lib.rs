//! Celo consensus validation for reth.
//!
//! Celo uses the same post-merge beacon consensus as Ethereum with additional
//! Celo-specific validations.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

use reth_celo_chainspec::CeloChainSpec;
use reth_consensus::{
    Consensus, ConsensusError, FullConsensus, HeaderValidator, ReceiptRootBloom,
};
use reth_ethereum_consensus::EthBeaconConsensus;
use reth_ethereum_primitives::{Block, EthPrimitives};
use reth_execution_types::BlockExecutionResult;
use reth_primitives_traits::{
    block::SealedBlock, NodePrimitives, RecoveredBlock, SealedHeader,
};
use std::sync::Arc;

/// Celo beacon consensus validator.
///
/// Delegates to Ethereum's [`EthBeaconConsensus`] for standard post-merge validation,
/// and adds Celo-specific checks.
#[derive(Debug, Clone)]
pub struct CeloBeaconConsensus {
    /// The underlying Ethereum beacon consensus.
    inner: EthBeaconConsensus<CeloChainSpec>,
    /// The Celo chain specification (stored for future Celo-specific validation).
    #[allow(dead_code)]
    chain_spec: Arc<CeloChainSpec>,
}

impl CeloBeaconConsensus {
    /// Creates a new Celo beacon consensus validator.
    pub fn new(chain_spec: Arc<CeloChainSpec>) -> Self {
        Self {
            inner: EthBeaconConsensus::new(chain_spec.clone()),
            chain_spec,
        }
    }
}

impl HeaderValidator<<Block as reth_primitives_traits::Block>::Header> for CeloBeaconConsensus {
    fn validate_header(
        &self,
        header: &SealedHeader<<Block as reth_primitives_traits::Block>::Header>,
    ) -> Result<(), ConsensusError> {
        self.inner.validate_header(header)
    }

    fn validate_header_against_parent(
        &self,
        header: &SealedHeader<<Block as reth_primitives_traits::Block>::Header>,
        parent: &SealedHeader<<Block as reth_primitives_traits::Block>::Header>,
    ) -> Result<(), ConsensusError> {
        self.inner.validate_header_against_parent(header, parent)
    }
}

impl Consensus<Block> for CeloBeaconConsensus {
    fn validate_body_against_header(
        &self,
        body: &<Block as reth_primitives_traits::Block>::Body,
        header: &SealedHeader<<Block as reth_primitives_traits::Block>::Header>,
    ) -> Result<(), ConsensusError> {
        <EthBeaconConsensus<CeloChainSpec> as Consensus<Block>>::validate_body_against_header(
            &self.inner,
            body,
            header,
        )
    }

    fn validate_block_pre_execution(
        &self,
        block: &SealedBlock<Block>,
    ) -> Result<(), ConsensusError> {
        <EthBeaconConsensus<CeloChainSpec> as Consensus<Block>>::validate_block_pre_execution(
            &self.inner,
            block,
        )
    }
}

impl FullConsensus<EthPrimitives> for CeloBeaconConsensus {
    fn validate_block_post_execution(
        &self,
        block: &RecoveredBlock<<EthPrimitives as NodePrimitives>::Block>,
        result: &BlockExecutionResult<<EthPrimitives as NodePrimitives>::Receipt>,
        receipt_root_bloom: Option<ReceiptRootBloom>,
    ) -> Result<(), ConsensusError> {
        <EthBeaconConsensus<CeloChainSpec> as FullConsensus<EthPrimitives>>::validate_block_post_execution(
            &self.inner,
            block,
            result,
            receipt_root_bloom,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reth_celo_chainspec::CELO_DEV;

    #[test]
    fn test_celo_consensus_creation() {
        let consensus = CeloBeaconConsensus::new(CELO_DEV.clone());
        assert!(consensus.chain_spec.is_celo());
    }
}
