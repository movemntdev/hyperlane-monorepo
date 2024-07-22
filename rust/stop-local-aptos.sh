ps aux | grep aptos | grep "node run-local-testnet" | awk '{print $2}' | xargs kill
