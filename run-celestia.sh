#!/usr/bin/env bash

docker run --network host -e NODE_TYPE=$NODE_TYPE -e P2P_NETWORK=$NETWORK \
    --name celestia-node \
    -v ./volumes/celestia:/home/celestia \
    ghcr.io/celestiaorg/celestia-node:v0.14.0-rc2 \
    celestia $NODE_TYPE start --core.ip $RPC_URL --p2p.network $NETWORK

# eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJBbGxvdyI6WyJwdWJsaWMiLCJyZWFkIiwid3JpdGUiLCJhZG1pbiJdfQ.ut1X4u9XG5cbV0yaRAKfGp9xWVrz3NoEPGGRch13dFU
# address:
# celestia14aa9asfwdheasrc5q8kl4vz7kp4k6leaz7wuph
