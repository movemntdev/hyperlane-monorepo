#!/bin/bash -e

# Variables
RPC_URL="http://localhost:4242"
PRIVATE_KEY=$(jq -r '.private_keys[0]' "./process-compose/anvil.json")
MAILBOX_ADDRESS=""

# Navigate to the Solidity directory
cd ./solidity

# Deploy Router.sol
forge create --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY" ./contracts/client/Router.sol:Router
echo "Deployed Router.sol!"

# Deploy GasRouter.sol
forge create --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY" ./contracts/client/GasRouter.sol:GasRouter
echo "Deployed GasRouter.sol!"

# Deploy MailboxClient.sol
forge create --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY" ./contracts/client/MailboxClient.sol:MailboxClient
echo "Deployed MailboxClient.sol!"

# Deploy Mailbox.sol and capture the address
MAILBOX_ADDRESS=$(forge create --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY" ./contracts/Mailbox.sol:Mailbox --constructor-args 666 | awk '/Deployed to:/ {print $3}')
echo "Deployed Mailbox.sol! to $MAILBOX_ADDRESS"

# Deploy InterchainGasPaymaster.sol
forge create --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY" ./contracts/hooks/igp/InterchainGasPaymaster.sol:InterchainGasPaymaster
echo "Deployed InterchainGasPaymaster.sol! (hook)"

# Deploy StorageGasOracle.sol
forge create --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY" ./contracts/hooks/igp/StorageGasOracle.sol:StorageGasOracle
echo "Deployed StorageGasOracle.sol!"

# Deploy MerkleTreeHook.sol with the MAILBOX_ADDRESS
forge create --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY" ./contracts/hooks/MerkleTreeHook.sol:MerkleTreeHook --constructor-args "$MAILBOX_ADDRESS"
echo "Deployed MerkleTreeHook.sol!"

