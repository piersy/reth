//! Celo execution client built on reth.
//!
//! This binary provides a Celo-compatible execution client using reth's
//! modular architecture. It integrates:
//! - celo-revm for EVM-level Celo modifications
//! - Celo chain specs for network configuration
//! - Fee currency support (CIP-64) at the EVM level
//! - Token duality via the transfer precompile

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use clap::Parser;
use reth_celo_chainspec::CeloChainSpec;
use reth_celo_consensus::CeloBeaconConsensus;
use reth_celo_evm::CeloEvmConfig;
use reth_celo_node::CeloNode;
use reth_ethereum_cli::Cli;
use std::sync::Arc;
use tracing::info;

fn main() {
    reth_cli_util::sigsegv_handler::install();

    // Enable backtraces unless a RUST_BACKTRACE value has already been explicitly provided.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        unsafe { std::env::set_var("RUST_BACKTRACE", "1") };
    }

    let components = |spec: Arc<CeloChainSpec>| {
        (CeloEvmConfig::new(spec.clone()), Arc::new(CeloBeaconConsensus::new(spec)))
    };

    if let Err(err) =
        Cli::<reth_celo_cli::CeloChainSpecParser>::parse().run_with_components::<CeloNode>(
            components,
            async move |builder, _| {
                info!(target: "reth::cli", "Launching Celo node");
                let handle = builder.node(CeloNode::default()).launch().await?;
                handle.wait_for_node_exit().await
            },
        )
    {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}
