//! Celo CLI chain specification parser for reth.
//!
//! Provides the [`CeloChainSpecParser`] that can parse Celo chain names
//! (mainnet, sepolia, dev) or custom genesis JSON files.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

use reth_celo_chainspec::{CeloChainSpec, CELO_DEV, CELO_MAINNET, CELO_SEPOLIA};
use std::sync::Arc;

/// Known Celo chain names.
pub const SUPPORTED_CHAINS: &[&str] = &["celo", "celo-mainnet", "celo-sepolia", "dev"];

/// Parses a chain name or genesis JSON path into a [`CeloChainSpec`].
pub fn chain_value_parser(s: &str) -> eyre::Result<Arc<CeloChainSpec>, eyre::Error> {
    Ok(match s {
        "celo" | "celo-mainnet" => CELO_MAINNET.clone(),
        "celo-sepolia" => CELO_SEPOLIA.clone(),
        "dev" => CELO_DEV.clone(),
        _ => {
            // Try parsing as a genesis JSON file
            let genesis = reth_celo_chainspec::parse_genesis_json(s)?;
            Arc::new(CeloChainSpec::from_genesis(genesis))
        }
    })
}

/// Chain spec parser for Celo networks.
///
/// Implements `ChainSpecParser` to integrate with reth's CLI system.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct CeloChainSpecParser;

impl reth_cli::chainspec::ChainSpecParser for CeloChainSpecParser {
    type ChainSpec = CeloChainSpec;

    const SUPPORTED_CHAINS: &'static [&'static str] = SUPPORTED_CHAINS;

    fn parse(s: &str) -> eyre::Result<Arc<Self::ChainSpec>> {
        chain_value_parser(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reth_celo_forks::chain_id;
    use reth_chainspec::EthChainSpec;

    #[test]
    fn test_parse_celo_mainnet() {
        let spec = chain_value_parser("celo").unwrap();
        assert_eq!(spec.chain().id(), chain_id::CELO_MAINNET);
    }

    #[test]
    fn test_parse_celo_sepolia() {
        let spec = chain_value_parser("celo-sepolia").unwrap();
        assert_eq!(spec.chain().id(), chain_id::CELO_SEPOLIA);
    }

    #[test]
    fn test_parse_dev() {
        let spec = chain_value_parser("dev").unwrap();
        assert!(spec.is_celo());
    }
}
