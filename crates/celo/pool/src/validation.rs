//! Celo-specific transaction pool validation.
//!
//! Extends reth's standard transaction validation with fee currency checks.

use alloy_primitives::Address;
use std::collections::HashMap;

/// Exchange rate for a fee currency, expressed as a rational number
/// (numerator/denominator) relative to native CELO.
///
/// Example: If 1 CELO = 2 USD, the rate for USD is numerator=2, denominator=1.
#[derive(Debug, Clone, Copy)]
pub struct ExchangeRate {
    /// Numerator of the exchange rate.
    pub numerator: u128,
    /// Denominator of the exchange rate.
    pub denominator: u128,
}

impl ExchangeRate {
    /// Creates a new exchange rate.
    pub const fn new(numerator: u128, denominator: u128) -> Self {
        Self { numerator, denominator }
    }

    /// Converts a value from native CELO to this fee currency.
    pub fn to_fee_currency(&self, native_value: u128) -> u128 {
        native_value * self.numerator / self.denominator
    }

    /// Converts a value from this fee currency to native CELO.
    pub fn to_native(&self, fee_currency_value: u128) -> u128 {
        fee_currency_value * self.denominator / self.numerator
    }
}

/// Map from fee currency address to exchange rate.
pub type ExchangeRates = HashMap<Address, ExchangeRate>;

/// Compares two fee values that may be in different currencies.
///
/// Returns:
/// - `Ok(1)` if val1 > val2 (in native terms)
/// - `Ok(0)` if val1 == val2
/// - `Ok(-1)` if val1 < val2
/// - `Err(())` if a fee currency is not registered
///
/// Port of `exchange.CompareValue` from Go.
pub fn compare_value(
    rates: &ExchangeRates,
    val1: u128,
    currency1: Option<&Address>,
    val2: u128,
    currency2: Option<&Address>,
) -> Result<i8, ()> {
    let native1 = to_native(rates, val1, currency1)?;
    let native2 = to_native(rates, val2, currency2)?;

    Ok(match native1.cmp(&native2) {
        std::cmp::Ordering::Greater => 1,
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
    })
}

/// Converts a value to native CELO, using the exchange rate if a fee currency is provided.
fn to_native(
    rates: &ExchangeRates,
    value: u128,
    currency: Option<&Address>,
) -> Result<u128, ()> {
    match currency {
        None => Ok(value),
        Some(addr) => {
            let rate = rates.get(addr).ok_or(())?;
            // Convert from fee currency to native: value * denominator / numerator
            Ok(value * rate.denominator / rate.numerator)
        }
    }
}

/// Returns true if the given fee currency is allowed (registered in exchange rates).
///
/// - `None` (native CELO) is always allowed.
/// - An address must be present in the exchange rates map.
///
/// Port of `common.IsCurrencyAllowed` from Go.
pub fn is_currency_allowed(rates: &ExchangeRates, fee_currency: Option<&Address>) -> bool {
    match fee_currency {
        None => true,
        Some(addr) => rates.contains_key(addr),
    }
}

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

    fn curr_a() -> Address {
        Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x0a])
    }

    fn curr_b() -> Address {
        Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x0b])
    }

    fn curr_x() -> Address {
        Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x0f])
    }

    fn test_rates() -> ExchangeRates {
        let mut rates = HashMap::new();
        rates.insert(curr_a(), ExchangeRate::new(47, 100));
        rates.insert(curr_b(), ExchangeRate::new(45, 100));
        rates
    }

    // Port of TestIsCurrencyAllowed from Go

    #[test]
    fn test_no_fee_currency_always_allowed() {
        let rates = test_rates();
        assert!(is_currency_allowed(&rates, None));
    }

    #[test]
    fn test_valid_fee_currency_allowed() {
        let rates = test_rates();
        assert!(is_currency_allowed(&rates, Some(&curr_a())));
    }

    #[test]
    fn test_invalid_fee_currency_not_allowed() {
        let rates = test_rates();
        assert!(!is_currency_allowed(&rates, Some(&curr_x())));
    }

    // Port of TestCompareFees from Go

    #[test]
    fn test_same_amount_native_currency() {
        let rates = test_rates();
        assert_eq!(compare_value(&rates, 1, None, 1, None).unwrap(), 0);
    }

    #[test]
    fn test_different_amounts_native_currency_1() {
        let rates = test_rates();
        assert_eq!(compare_value(&rates, 2, None, 1, None).unwrap(), 1);
    }

    #[test]
    fn test_different_amounts_native_currency_2() {
        let rates = test_rates();
        assert_eq!(compare_value(&rates, 1, None, 5, None).unwrap(), -1);
    }

    #[test]
    fn test_same_amount_mixed_currency() {
        let rates = test_rates();
        // 1 native vs 1 currA: currA rate is 47/100, so 1 currA = 100/47 ≈ 2.1 native
        // So 1 native < 1 currA in native terms
        assert_eq!(compare_value(&rates, 1, None, 1, Some(&curr_a())).unwrap(), -1);
    }

    #[test]
    fn test_different_amounts_mixed_currency_1() {
        let rates = test_rates();
        // 100 native vs 47 currA: 47 currA = 47 * 100/47 = 100 native
        assert_eq!(compare_value(&rates, 100, None, 47, Some(&curr_a())).unwrap(), 0);
    }

    #[test]
    fn test_different_amounts_mixed_currency_2() {
        let rates = test_rates();
        // 45 currB vs 100 native: 45 currB = 45 * 100/45 = 100 native
        assert_eq!(compare_value(&rates, 45, Some(&curr_b()), 100, None).unwrap(), 0);
    }

    #[test]
    fn test_same_amount_same_currency() {
        let rates = test_rates();
        assert_eq!(
            compare_value(&rates, 1, Some(&curr_a()), 1, Some(&curr_a())).unwrap(),
            0
        );
    }

    #[test]
    fn test_different_amounts_same_currency_1() {
        let rates = test_rates();
        assert_eq!(
            compare_value(&rates, 3, Some(&curr_a()), 1, Some(&curr_a())).unwrap(),
            1
        );
    }

    #[test]
    fn test_different_amounts_same_currency_2() {
        let rates = test_rates();
        assert_eq!(
            compare_value(&rates, 1, Some(&curr_a()), 7, Some(&curr_a())).unwrap(),
            -1
        );
    }

    #[test]
    fn test_different_amounts_different_currencies_equal() {
        let rates = test_rates();
        // 47 currA vs 45 currB:
        // 47 currA = 47 * 100/47 = 100 native
        // 45 currB = 45 * 100/45 = 100 native
        assert_eq!(
            compare_value(&rates, 47, Some(&curr_a()), 45, Some(&curr_b())).unwrap(),
            0
        );
    }

    #[test]
    fn test_different_amounts_different_currencies_greater() {
        let rates = test_rates();
        // 48 currA vs 45 currB:
        // 48 currA = 48 * 100/47 = 102 native (truncated)
        // 45 currB = 45 * 100/45 = 100 native
        assert_eq!(
            compare_value(&rates, 48, Some(&curr_a()), 45, Some(&curr_b())).unwrap(),
            1
        );
    }

    #[test]
    fn test_different_amounts_different_currencies_less() {
        let rates = test_rates();
        // 47 currA vs 46 currB:
        // 47 currA = 47 * 100/47 = 100 native
        // 46 currB = 46 * 100/45 = 102 native (truncated)
        assert_eq!(
            compare_value(&rates, 47, Some(&curr_a()), 46, Some(&curr_b())).unwrap(),
            -1
        );
    }

    #[test]
    fn test_unregistered_fee_currency_error() {
        let rates = test_rates();
        assert!(compare_value(&rates, 1, Some(&curr_a()), 1, Some(&curr_x())).is_err());
    }

    #[test]
    fn test_exchange_rate_conversion() {
        let rate = ExchangeRate::new(2, 1);
        // 1 CELO = 2 USD
        assert_eq!(rate.to_fee_currency(100), 200);
        assert_eq!(rate.to_native(200), 100);
    }

    #[test]
    fn test_currency_registration() {
        let currency = Address::random();
        let validator = CeloPoolValidator::new(vec![currency]);

        assert!(validator.is_registered_currency(&currency));
        assert!(!validator.is_registered_currency(&Address::random()));
    }
}
