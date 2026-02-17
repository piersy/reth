//! Celo payload builder for reth.
//!
//! This crate provides the Celo-specific payload builder that extends Ethereum's
//! payload building with support for:
//! - CIP-64 fee currency transactions
//! - Multi-gas pool management per fee currency
//! - Fee currency blocklisting
//!
//! The payload builder is responsible for constructing execution payloads from the
//! transaction pool, respecting Celo-specific constraints.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

use reth_ethereum_payload_builder::EthereumPayloadBuilder;

/// Re-export the Ethereum payload builder.
///
/// Celo currently uses the Ethereum payload builder as a base.
/// Fee currency handling is done at the EVM level by celo-revm,
/// so the payload builder primarily needs to handle transaction
/// ordering and multi-gas pool management.
///
/// TODO: Extend with Celo-specific multi-gas pool logic for fee currencies.
pub type CeloPayloadBuilder = EthereumPayloadBuilder;
