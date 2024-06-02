#!/bin/bash -e

DEPLOYER_KEY=$(jq -r '.private_keys[0]' "./process-compose/anvil.json")

hyperlane deploy core \
    --targets anvil1,aptoslocalnet1 \ # all the chains you want to bridge between
    --ism $MULTISIG_CONFIG_FILE \ # path to ism.yaml config e.g. ./configs/ism.yaml
    --key $DEPLOYER_KEY \ # (optional) your private key to pay for transactions; can also be provided via HYP_KEY env variable
    --registry \ ./configs
    --overrides # (optional) path to a override registry; defaults to the local ./ path

