//! Celo transaction pool validation with fee currency support.
//!
//! This crate provides Celo-specific transaction pool validation that extends
//! reth's standard validation with:
//!
//! - **Fee currency validation**: Ensures fee currency addresses are registered
//!   in the FeeCurrencyDirectory contract
//! - **Exchange rate conversion**: Converts gas prices between native CELO and
//!   fee currencies for proper ordering
//! - **Multi-gas pool management**: Separates gas accounting per fee currency
//!   during block building
//! - **Currency blocklist**: Temporarily blocks malfunctioning fee currencies

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod blocklist;
mod multi_gas_pool;
mod validation;

pub use blocklist::CurrencyBlocklist;
pub use multi_gas_pool::MultiGasPool;
pub use validation::CeloPoolValidator;
