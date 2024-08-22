#!/bin/bash

# Function to generate a random number between a specified range
function random_between() {
  local min=$1
  local max=$2
  echo $(( (RANDOM % (max-min+1)) + min ))
}

# Generate transactions
for i in {1..10}
do
  num_tx=$(random_between 50 150)

  for j in $(seq 1 $num_tx)
  do
    # Generate a new address
    new_address=$(./bitcoin-27.1/bin/bitcoin-cli -regtest getnewaddress)

    # Create an unfunded raw transaction
    unfunded_tx=$(./bitcoin-27.1/bin/bitcoin-cli -regtest createrawtransaction "[]" "{\"$new_address\":0.005}")

    # Calculate a random fee rate
    fee_factor=$(random_between 0 28)
    rand_fee=$(echo "0.00001 * (1.1892 ^ $fee_factor)" | bc -l)
    rand_fee=$(printf "%.8f" $rand_fee)

    # Fund the raw transaction with the calculated fee rate
    funded_tx=$(./bitcoin-27.1/bin/bitcoin-cli -regtest fundrawtransaction "$unfunded_tx" "{\"feeRate\":\"$rand_fee\"}")

    # Sign the raw transaction with the wallet
    signed_tx=$(./bitcoin-27.1/bin/bitcoin-cli -regtest signrawtransactionwithwallet "$(echo $funded_tx | jq -r '.hex')")

    # Send the signed transaction
    ./bitcoin-27.1/bin/bitcoin-cli -regtest sendrawtransaction "$(echo $signed_tx | jq -r '.hex')"
  done

  # Generate 1 block to the specified address
  ./bitcoin-27.1/bin/bitcoin-cli -regtest generatetoaddress 1 "bcrt1qvct98j4rfpfwrrvwhrts47re03z3zhkh26pyxj"
done