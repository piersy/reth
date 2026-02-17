#!/bin/bash
set -eo pipefail

source shared.sh
prepare_node

(cd debug-fee-currency && forge build --out $PWD/out $PWD)
export COMPILED_TEST_CONTRACT=../debug-fee-currency/out/DebugFeeCurrency.sol/DebugFeeCurrency.json
(cd js-tests && ./node_modules/mocha/bin/mocha.js test_viem_smoketest.mjs --timeout 25000 --exit)
