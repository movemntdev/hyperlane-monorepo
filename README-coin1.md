# Set up hyperlane v2 for aptos

## Build binaries
```
cd rust
cargo build
```

## Run local tests
```
./run-local-aptos.sh
./init-local-aptos.sh
./run-validator.sh 0 # run validator for aptoslocal1
cd ../move/e2e
./init_states.sh send_hello_ln1_to_ln2 # send hello from aptos1 to aptos2
./init_states.sh send_hello_ln2_to_ln1
```


