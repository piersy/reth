//! Celo-specific transaction pool validation.
//!
//! Extends reth's standard transaction validation with fee currency checks.

use alloy_primitives::Address;

/// Celo transaction pool validator.
///
/// Wraps reth's standard validation and adds:
/// - Fee currency whitelist checking
/// - Fee currency balance validation
/// - Base fee floor validation (pre-Jovian)
///
/// TODO: Implement full validation once celo-revm integration is complete.
#[derive(Debug, Clone)]
pub struct CeloPoolValidator {
    /// Set of registered fee currency addresses.
    /// Loaded from the FeeCurrencyDirectory contract.
    registered_currencies: Vec<Address>,
}

impl CeloPoolValidator {
    /// Creates a new Celo pool validator.
    pub fn new(registered_currencies: Vec<Address>) -> Self {
        Self { registered_currencies }
    }

    /// Returns true if the given address is a registered fee currency.
    pub fn is_registered_currency(&self, address: &Address) -> bool {
        self.registered_currencies.contains(address)
    }

    /// Updates the set of registered fee currencies.
    pub fn update_currencies(&mut self, currencies: Vec<Address>) {
        self.registered_currencies = currencies;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_currency_registration() {
        let currency = Address::random();
        let validator = CeloPoolValidator::new(vec![currency]);

        assert!(validator.is_registered_currency(&currency));
        assert!(!validator.is_registered_currency(&Address::random()));
    }
}
