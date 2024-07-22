# initialize aptos states
set -e
set -x
#

pushd ../move/e2e/
./init_states.sh init_ln1_modules
./init_states.sh init_ln2_modules
#./init_states.sh send_hello_ln1_to_ln2
#./init_states.sh send_hello_ln2_to_ln1




