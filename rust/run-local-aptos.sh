set -e
set -x
#
# run local aptos node
/home/coin1/.local/bin/aptos node run-local-testnet --with-faucet --faucet-port 8081 --force-restart --assume-yes > /tmp/aptos-local-node.log 2>&1&

sleep 20
pushd ../move/e2e/
./compile-and-deploy.sh




