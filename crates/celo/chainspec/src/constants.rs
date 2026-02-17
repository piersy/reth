//! Celo chain constants including contract addresses and protocol parameters.

use alloy_primitives::Address;

/// The CELO token contract address on mainnet.
pub const CELO_TOKEN_MAINNET: Address =
    Address::new(hex_literal::hex!("471ece3750da237f93b8e339c536989b8978a438"));

/// The FeeHandler contract address on mainnet (base fee recipient).
pub const FEE_HANDLER_MAINNET: Address =
    Address::new(hex_literal::hex!("cd437749e43a154c07f3553504c68fbfd56b8778"));

/// The FeeCurrencyDirectory contract address on mainnet.
pub const FEE_CURRENCY_DIRECTORY_MAINNET: Address =
    Address::new(hex_literal::hex!("15F344b9E6c3Cb6F0376A36A64928b13F62C6276"));

/// The CELO token contract address on Sepolia testnet.
pub const CELO_TOKEN_SEPOLIA: Address =
    Address::new(hex_literal::hex!("471ece3750da237f93b8e339c536989b8978a438"));

/// The FeeHandler contract address on Sepolia testnet.
pub const FEE_HANDLER_SEPOLIA: Address =
    Address::new(hex_literal::hex!("cd437749e43a154c07f3553504c68fbfd56b8778"));

/// The FeeCurrencyDirectory contract address on Sepolia testnet.
pub const FEE_CURRENCY_DIRECTORY_SEPOLIA: Address =
    Address::new(hex_literal::hex!("15F344b9E6c3Cb6F0376A36A64928b13F62C6276"));

/// The transfer precompile address (0x00...00fd) for token duality.
/// This allows CELO to function as both a native currency and an ERC-20 token.
pub const TRANSFER_PRECOMPILE_ADDRESS: Address =
    Address::new(hex_literal::hex!("00000000000000000000000000000000000000fd"));

/// The system address used for system calls (Address::ZERO).
pub const SYSTEM_ADDRESS: Address = Address::ZERO;

/// Maximum code size in bytes (64KB, matching Ethereum).
pub const MAX_CODE_SIZE: usize = 65536;

/// Gas cost for the transfer precompile.
pub const TRANSFER_PRECOMPILE_GAS: u64 = 9000;

/// Default fraction of block gas limit allocated per fee currency pool.
/// Used in multi-gas pool system during block building.
pub const DEFAULT_FEE_CURRENCY_GAS_FRACTION: f64 = 0.9;

/// Time in seconds before a blocked fee currency is automatically unblocked.
pub const BLOCKLIST_EVICTION_TIMEOUT_SECS: u64 = 7200; // 2 hours
