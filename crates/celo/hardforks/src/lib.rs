//! Celo hardfork types for reth.
//!
//! This crate defines Celo-specific hardforks that extend the Ethereum and OP Stack hardforks.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]

pub use reth_ethereum_forks::*;

mod hardforks;
pub use hardforks::*;
