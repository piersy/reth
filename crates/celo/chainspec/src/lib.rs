//! Celo chain specification for reth.
//!
//! Defines chain specs for Celo networks (mainnet, Sepolia, dev) including
//! hardfork activation conditions and chain-specific configuration.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

use alloy_chains::Chain;
use alloy_consensus::Header;
use alloy_eips::eip7840::BlobParams;
use alloy_evm::eth::spec::EthExecutorSpec;
use alloy_genesis::Genesis;
use alloy_primitives::{Address, B256, U256};
use once_cell::sync::Lazy;
use reth_chainspec::{
    BaseFeeParams, ChainSpec, DepositContract, EthChainSpec, EthereumHardfork, EthereumHardforks,
    ForkCondition, Hardforks,
};
use reth_celo_forks::{chain_id, CeloHardfork, ForkFilter, ForkId, Head};
use reth_network_peers::NodeRecord;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

mod constants;
pub use constants::*;

/// Celo chain specification wrapping reth's [`ChainSpec`] with Celo-specific hardforks.
#[derive(Debug, Clone)]
pub struct CeloChainSpec {
    /// The inner Ethereum chain spec.
    pub inner: ChainSpec,
}

impl CeloChainSpec {
    /// Creates a new Celo chain spec wrapping the given [`ChainSpec`].
    pub fn new(inner: ChainSpec) -> Self {
        Self { inner }
    }

    /// Creates a Celo chain spec from a genesis configuration.
    pub fn from_genesis(genesis: Genesis) -> Self {
        let extra = genesis
            .config
            .extra_fields
            .deserialize_as::<CeloHardforkConfig>()
            .unwrap_or_default();

        let mut inner = ChainSpec::from_genesis(genesis);

        // Insert Celo-specific hardforks
        if let Some(ts) = extra.gingerbread_time {
            inner.hardforks.insert(CeloHardfork::Gingerbread, ForkCondition::Timestamp(ts));
        }
        if let Some(ts) = extra.cel2_time {
            inner.hardforks.insert(CeloHardfork::Cel2, ForkCondition::Timestamp(ts));
        }
        if let Some(ts) = extra.jovian_time {
            inner.hardforks.insert(CeloHardfork::Jovian, ForkCondition::Timestamp(ts));
        }

        Self { inner }
    }

    /// Returns true if the Gingerbread hardfork is active at the given timestamp.
    pub fn is_gingerbread_active_at_timestamp(&self, timestamp: u64) -> bool {
        self.inner
            .hardforks
            .is_fork_active_at_timestamp(CeloHardfork::Gingerbread, timestamp)
    }

    /// Returns true if the Cel2 hardfork is active at the given timestamp.
    pub fn is_cel2_active_at_timestamp(&self, timestamp: u64) -> bool {
        self.inner.hardforks.is_fork_active_at_timestamp(CeloHardfork::Cel2, timestamp)
    }

    /// Returns true if the Jovian hardfork is active at the given timestamp.
    pub fn is_jovian_active_at_timestamp(&self, timestamp: u64) -> bool {
        self.inner.hardforks.is_fork_active_at_timestamp(CeloHardfork::Jovian, timestamp)
    }

    /// Returns true if this is a known Celo chain.
    pub fn is_celo(&self) -> bool {
        reth_celo_forks::is_celo_chain(self.inner.chain().id())
    }
}

/// Celo-specific hardfork activation times, deserialized from genesis extra fields.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CeloHardforkConfig {
    /// Timestamp for the Gingerbread hardfork activation.
    pub gingerbread_time: Option<u64>,
    /// Timestamp for the Cel2 hardfork activation.
    pub cel2_time: Option<u64>,
    /// Timestamp for the Jovian hardfork activation.
    pub jovian_time: Option<u64>,
}

// Implement Hardforks to delegate to inner ChainSpec
impl Hardforks for CeloChainSpec {
    fn fork<H: reth_celo_forks::Hardfork>(&self, fork: H) -> ForkCondition {
        self.inner.fork(fork)
    }

    fn forks_iter(&self) -> impl Iterator<Item = (&dyn reth_celo_forks::Hardfork, ForkCondition)> {
        self.inner.forks_iter()
    }

    fn fork_id(&self, head: &Head) -> ForkId {
        self.inner.fork_id(head)
    }

    fn latest_fork_id(&self) -> ForkId {
        self.inner.latest_fork_id()
    }

    fn fork_filter(&self, head: Head) -> ForkFilter {
        self.inner.fork_filter(head)
    }
}

impl EthChainSpec for CeloChainSpec {
    type Header = Header;

    fn chain(&self) -> Chain {
        self.inner.chain()
    }

    fn base_fee_params_at_timestamp(&self, timestamp: u64) -> BaseFeeParams {
        self.inner.base_fee_params_at_timestamp(timestamp)
    }

    fn blob_params_at_timestamp(&self, timestamp: u64) -> Option<BlobParams> {
        self.inner.blob_params_at_timestamp(timestamp)
    }

    fn deposit_contract(&self) -> Option<&DepositContract> {
        self.inner.deposit_contract()
    }

    fn genesis_hash(&self) -> B256 {
        self.inner.genesis_hash()
    }

    fn prune_delete_limit(&self) -> usize {
        self.inner.prune_delete_limit()
    }

    fn display_hardforks(&self) -> Box<dyn core::fmt::Display> {
        Box::new(self.inner.display_hardforks())
    }

    fn genesis_header(&self) -> &Self::Header {
        self.inner.genesis_header()
    }

    fn genesis(&self) -> &Genesis {
        self.inner.genesis()
    }

    fn bootnodes(&self) -> Option<Vec<NodeRecord>> {
        self.inner.bootnodes()
    }

    fn final_paris_total_difficulty(&self) -> Option<U256> {
        self.inner.final_paris_total_difficulty()
    }
}

impl EthereumHardforks for CeloChainSpec {
    fn ethereum_fork_activation(&self, fork: EthereumHardfork) -> ForkCondition {
        self.inner.ethereum_fork_activation(fork)
    }
}

impl EthExecutorSpec for CeloChainSpec {
    fn deposit_contract_address(&self) -> Option<Address> {
        self.inner.deposit_contract_address()
    }
}

/// Celo Mainnet chain spec.
pub static CELO_MAINNET: Lazy<Arc<CeloChainSpec>> = Lazy::new(|| {
    let mut spec = ChainSpec::builder()
        .chain(Chain::from_id(chain_id::CELO_MAINNET))
        .genesis(celo_mainnet_genesis())
        .london_activated()
        .paris_activated()
        .shanghai_activated()
        .cancun_activated()
        .build();

    // Add Celo-specific hardforks
    spec.hardforks.insert(CeloHardfork::Gingerbread, ForkCondition::Timestamp(0));
    spec.hardforks.insert(CeloHardfork::Cel2, ForkCondition::Timestamp(0));

    // Isthmus timestamp for mainnet: July 9, 2025
    spec.hardforks
        .insert(EthereumHardfork::Prague, ForkCondition::Timestamp(1752073200));

    Arc::new(CeloChainSpec::new(spec))
});

/// Celo Sepolia testnet chain spec.
pub static CELO_SEPOLIA: Lazy<Arc<CeloChainSpec>> = Lazy::new(|| {
    let mut spec = ChainSpec::builder()
        .chain(Chain::from_id(chain_id::CELO_SEPOLIA))
        .genesis(celo_sepolia_genesis())
        .london_activated()
        .paris_activated()
        .shanghai_activated()
        .cancun_activated()
        .build();

    // Add Celo-specific hardforks
    spec.hardforks.insert(CeloHardfork::Gingerbread, ForkCondition::Timestamp(0));
    spec.hardforks.insert(CeloHardfork::Cel2, ForkCondition::Timestamp(0));

    Arc::new(CeloChainSpec::new(spec))
});

/// Celo dev chain spec for local development.
pub static CELO_DEV: Lazy<Arc<CeloChainSpec>> = Lazy::new(|| {
    let mut spec = ChainSpec::builder()
        .chain(Chain::from_id(chain_id::CELO_SEPOLIA))
        .genesis(Genesis::clique_genesis(1, Default::default()))
        .london_activated()
        .paris_activated()
        .shanghai_activated()
        .cancun_activated()
        .build();

    // All Celo hardforks active from genesis in dev mode
    spec.hardforks.insert(CeloHardfork::Gingerbread, ForkCondition::Timestamp(0));
    spec.hardforks.insert(CeloHardfork::Cel2, ForkCondition::Timestamp(0));
    spec.hardforks.insert(CeloHardfork::Jovian, ForkCondition::Timestamp(0));

    Arc::new(CeloChainSpec::new(spec))
});

/// Creates a minimal Celo Mainnet genesis configuration.
fn celo_mainnet_genesis() -> Genesis {
    serde_json::from_str(include_str!("../res/genesis/mainnet.json"))
        .expect("Failed to parse Celo mainnet genesis")
}

/// Creates a minimal Celo Sepolia genesis configuration.
fn celo_sepolia_genesis() -> Genesis {
    serde_json::from_str(include_str!("../res/genesis/sepolia.json"))
        .expect("Failed to parse Celo Sepolia genesis")
}

/// Parses a genesis JSON file from a path string.
pub fn parse_genesis_json(path: &str) -> eyre::Result<Genesis> {
    let contents = std::fs::read_to_string(path)?;
    let genesis: Genesis = serde_json::from_str(&contents)?;
    Ok(genesis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn celo_mainnet_chain_id() {
        assert_eq!(CELO_MAINNET.chain().id(), chain_id::CELO_MAINNET);
    }

    #[test]
    fn celo_sepolia_chain_id() {
        assert_eq!(CELO_SEPOLIA.chain().id(), chain_id::CELO_SEPOLIA);
    }

    #[test]
    fn celo_mainnet_is_celo() {
        assert!(CELO_MAINNET.is_celo());
    }

    #[test]
    fn celo_dev_all_hardforks_active() {
        assert!(CELO_DEV.is_gingerbread_active_at_timestamp(0));
        assert!(CELO_DEV.is_cel2_active_at_timestamp(0));
        assert!(CELO_DEV.is_jovian_active_at_timestamp(0));
    }

    #[test]
    fn celo_chain_spec_from_genesis() {
        let genesis_json = r#"{
            "config": {
                "chainId": 42220,
                "homesteadBlock": 0,
                "eip155Block": 0,
                "eip158Block": 0,
                "gingerbreadTime": 1000,
                "cel2Time": 2000,
                "jovianTime": 3000
            },
            "alloc": {},
            "difficulty": "0x0",
            "gasLimit": "0x1c9c380"
        }"#;
        let genesis: Genesis = serde_json::from_str(genesis_json).unwrap();
        let spec = CeloChainSpec::from_genesis(genesis);

        assert!(spec.is_gingerbread_active_at_timestamp(1000));
        assert!(!spec.is_cel2_active_at_timestamp(1999));
        assert!(spec.is_cel2_active_at_timestamp(2000));
        assert!(spec.is_jovian_active_at_timestamp(3000));
    }
}
