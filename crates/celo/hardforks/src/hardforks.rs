//! Celo-specific hardfork definitions.
//!
//! These hardforks extend both Ethereum and OP Stack hardforks with Celo-specific
//! protocol upgrades.

use alloy_hardforks::hardfork;

// Define Celo-specific hardforks.
hardfork!(
    /// Celo chain hardforks.
    ///
    /// These represent Celo-specific protocol upgrades that activate at
    /// particular timestamps (since Celo is a post-merge OP Stack chain).
    CeloHardfork {
        /// Gingerbread hardfork - transition to OP Stack compatible block format.
        /// Enables EIP-1559 style base fee and changes block header encoding.
        Gingerbread,
        /// Cel2 hardfork - activation of Celo L2 features.
        /// Enables CIP-64 fee currency transactions and other L2-specific functionality.
        Cel2,
        /// Jovian hardfork - convergence toward standard OP Stack behavior.
        /// Transitions from Celo base fee floor to Optimism minBaseFee.
        /// Enables address warming for transfer precompile.
        Jovian,
    }
);

/// Celo chain IDs for known networks.
pub mod chain_id {
    /// Celo Mainnet chain ID.
    pub const CELO_MAINNET: u64 = 42220;
    /// Celo Sepolia testnet chain ID.
    pub const CELO_SEPOLIA: u64 = 11142220;
    /// Celo Chaos testnet chain ID.
    pub const CELO_CHAOS: u64 = 11162320;
}

/// Returns `true` if the given chain ID is a known Celo chain.
pub const fn is_celo_chain(chain_id: u64) -> bool {
    matches!(
        chain_id,
        chain_id::CELO_MAINNET | chain_id::CELO_SEPOLIA | chain_id::CELO_CHAOS
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_hardforks::ForkCondition;
    use reth_ethereum_forks::ChainHardforks;

    #[test]
    fn celo_hardfork_names() {
        assert_eq!(CeloHardfork::Gingerbread.name(), "Gingerbread");
        assert_eq!(CeloHardfork::Cel2.name(), "Cel2");
        assert_eq!(CeloHardfork::Jovian.name(), "Jovian");
    }

    #[test]
    fn celo_chain_id_detection() {
        assert!(is_celo_chain(42220));
        assert!(is_celo_chain(11142220));
        assert!(is_celo_chain(11162320));
        assert!(!is_celo_chain(1)); // Ethereum mainnet
        assert!(!is_celo_chain(10)); // OP mainnet
    }

    #[test]
    fn celo_hardforks_in_chain_hardforks() {
        let mut forks = ChainHardforks::default();
        forks.insert(CeloHardfork::Gingerbread, ForkCondition::Timestamp(1000));
        forks.insert(CeloHardfork::Cel2, ForkCondition::Timestamp(2000));
        forks.insert(CeloHardfork::Jovian, ForkCondition::Timestamp(3000));

        assert_eq!(forks.len(), 3);
        assert!(forks.is_fork_active_at_timestamp(CeloHardfork::Gingerbread, 1000));
        assert!(!forks.is_fork_active_at_timestamp(CeloHardfork::Cel2, 1999));
        assert!(forks.is_fork_active_at_timestamp(CeloHardfork::Cel2, 2000));
        assert!(forks.is_fork_active_at_timestamp(CeloHardfork::Jovian, 3000));
    }
}
