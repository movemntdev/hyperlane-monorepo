# Set up hyperlane v2 for aptos

## Preqrequisites

* Ubuntu 24.04 
* rust 1.76.0
* yarn 3.2.0
* node v20.14.0

### Install Node

This repository targets v20 of node. We recommend using [nvm](https://github.com/nvm-sh/nvm) to manage your node version.

To install nvm

```bash
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash
```

To install version 20

```bash
nvm install 20
nvm use 20
```

# install yarn
```bash
sudo apt install yarn
```

## Set up yarn
```
yarn install
yarn build
```

## Build binaries
```
cd rust
cargo build
```

## Run local tests manually
```
# in rust directory
export HYB_BASE_LOCAL_BIN=$HOME/.local/bin
./run-local-aptos.sh
./init-local-aptos.sh
./run-validator.sh 0 # run validator for aptoslocal1
cd ../move/e2e
./init_states.sh send_hello_ln1_to_ln2 # send hello from aptos1 to aptos2
./init_states.sh send_hello_ln2_to_ln1
```

# Run e2e test suite
```
# this runs aptos local client, 2 validators, relayer and test message scraper
# run-locally binary will send test messages between 2 set of aptos smart contracts,
# validators will sign them, relayer will deliver signatures and test scraper counts them
# in rust directory
export HYB_BASE_LOCAL_BIN=$HOME/.local/bin
./target/debug/run-locally
...
<E2E> E2E tests passed
...

```

run tests in infinite mode
```angular2html
export HYB_BASE_LOCAL_BIN=$HOME/.local/bin
HYB_BASE_LOOP=1  ./target/debug/run-locally

```


