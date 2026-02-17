#!/bin/bash
#shellcheck disable=SC2086
set -eo pipefail

source shared.sh
source debug-fee-currency/lib.sh

# Expect that the creditGasFees failed and is logged by reth-celo
tail -F -n 0 reth-celo.log >debug-fee-currency/reth-celo.intrinsic.log & # start log capture
trap 'kill %%' EXIT # kill bg tail job on exit
(
	sleep 0.2
	fee_currency=$(deploy_fee_currency false false true)

	# trigger the first failed call to the CreditFees(), causing the
	# currency to get temporarily blocklisted.
	# initial tx should not succeed, should have required a replacement transaction.
	cip_64_tx $fee_currency 1 true 2 | assert_cip_64_tx false

	sleep 2

	# since the fee currency is temporarily blocked,
	# this should NOT make the transaction execute anymore,
	# but invalidate the transaction earlier.
	# initial tx should not succeed, should have required a replacement transaction.
	cip_64_tx $fee_currency 1 true 2 | assert_cip_64_tx false

	cleanup_fee_currency $fee_currency
)
sleep 0.5
# although we sent a transaction wih faulty fee-currency twice,
# the EVM call should have been executed only once
if [ "$(grep -Ec "fee-currency EVM execution error.+fee-currency contract error during internal EVM call: surpassed maximum allowed intrinsic gas for CreditFees\(\) in fee-currency" debug-fee-currency/reth-celo.intrinsic.log)" -ne 1 ]; then exit 1; fi
