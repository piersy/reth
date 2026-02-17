#!/bin/bash
set -eo pipefail

SCRIPT_DIR=$(readlink -f "$(dirname "$0")")
source "$SCRIPT_DIR/shared.sh"

TEST_GLOB=$1

if [ -z $NETWORK ]; then
    ## Build reth-celo
    cd "$SCRIPT_DIR/.." || exit 1
    cargo build --bin reth-celo --release

    trap 'kill %%' EXIT # kill bg job at exit

    ## Start reth-celo in development mode
    target/release/reth-celo node --dev \
        --http --http.api eth,web3,net,admin \
        --chain dev \
        &>"$SCRIPT_DIR/reth-celo.log" &

    # Wait for reth-celo to be ready
    for _ in {1..20}; do
    	if cast block &>/dev/null; then
    		break
    	fi
    	sleep 0.5
    done

    ## Run tests
    echo "reth-celo ready, start tests"
fi

cd "$SCRIPT_DIR" || exit 1
# Send a warmup transaction before running tests
cast send --json --private-key "$ACC_PRIVKEY" "$TOKEN_ADDR" 'transfer(address to, uint256 value) returns (bool)' 0x000000000000000000000000000000000000dEaD 100 > /dev/null || true

failures=0
tests=0
echo "Globbing with \"$TEST_GLOB\""
for f in test_*"$TEST_GLOB"*; do
	echo "for file $f"
	if [[ -n $NETWORK ]]; then
		case $f in
		  # Skip tests that require a local network.
		  test_fee_currency_fails_on_credit.sh|test_fee_currency_fails_on_debit.sh|test_fee_currency_fails_intrinsic.sh|test_value_and_fee_currency_balance_check.sh|test_fee_currency_gas_estimation.sh|test_fee_currency_disable_blocking.sh|test_fee_currency_disable_and_enable_blocking.sh|test_fee_currency_unblock.sh)
		  echo "skipping file $f"
		  continue
		  ;;
	    esac
	else
		case $f in
		  # Skip test fee currency fails on credit, it seems broken
		  test_fee_currency_fails_on_credit.sh)
		  echo "skipping file $f"
		  continue
		  ;;
	    esac
	fi
	echo -e "\nRun $f"
	if "./$f"; then
		tput setaf 2 || true
		echo "PASS $f"
	else
		tput setaf 1 || true
		echo "FAIL $f"
		((failures++)) || true
	fi
	tput sgr0 || true
	((tests++)) || true
done

## Final summary
echo
if [[ $failures -eq 0 ]]; then
	tput setaf 2 || true
	echo All $tests tests succeeded!
else
	tput setaf 1 || true
	echo $failures/$tests failed.
fi
tput sgr0 || true
exit $failures
