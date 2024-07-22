

./run-relayer.sh 0 > /tmp/relayer0.log 2>&1 &
./run-relayer.sh 1 > /tmp/relayer1.log 2>&1 &
./run-validator.sh 0 > /tmp/validator0.log 2>&1 &
./run-validator.sh 1 > /tmp/validator1.log 2>&1 &
