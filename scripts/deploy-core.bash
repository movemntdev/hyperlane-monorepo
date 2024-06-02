#!/bin/bash -e

DEPLOYER_KEY=$(jq -r '.private_keys[0]' "./process-compose/anvil.json")
MULTISIG_CONFIG_FILE="./configs/ism.yaml"

npx hyperlane deploy core \
  --targets anvil,aptoslocalnet1 \
  --ism $MULTISIG_CONFIG_FILE \
  --key $DEPLOYER_KEY \
  --registry ./configs \
  --overrides " " \

