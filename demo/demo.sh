#!/usr/bin/env bash

# Demo Script for Project Feature

# Exit immediately if a command exits with a non-zero status.
set -e

NETWORK=arabica
NODE_TYPE=light
RPC_URL=validator-2.celestia-arabica-11.com
CELESTIA_CLIENT_API_NODE_URL=ws://localhost:26658
CELESTIA_CLIENT_API_PRIVATE_KEY="0xf55baf7c0e4e33b1d78fbf52f069c426bc36cff1aceb9bc8f45d14c07f034d73"
NODE_STORE=$HOME/.celestia-light-arabica-11

# Function to print script usage
usage() {
  echo "Usage: $0 [options]"
  echo "Options:"
  echo "  -h, --help    Show this help message and exit"
  exit 1
}

# Parse command line arguments
while [[ "$1" != "" ]]; do
  case $1 in
    -h | --help ) usage ;;
    * ) usage ;;
  esac
  shift
done

# Create directories for volumes
create_directories() {
  echo "Creating directories for volumes..."
  mkdir -p ../volumes/reth/data
  mkdir -p ../volumes/{postgres,celestia}
  echo "Directories created."
}

# Check if Docker is running
check_docker() {
  echo "Checking if Docker is running and the host network feature is enabled..."
  docker info > /dev/null 2>&1
  if [ $? -ne 0 ]; then
    echo "Docker does not seem to be running. Please start Docker and ensure the host network feature is enabled."
    exit 1
  fi
}

# Run Docker containers in detached mode
run_docker_containers() {
  echo "Running reth and postgres services in detached mode..."
  docker-compose up -d reth postgres
  echo "reth and postgres services are now running in detached mode."
}

# Create zksync_local database
create_zksync_local_db() {
  echo "Creating zksync_local database..."
  docker exec -i $(docker ps -q -f name=postgres) psql -h 127.0.0.1 -p 5432 -U postgres -d postgres -c "CREATE DATABASE zksync_local;" <<< "notsecurepassword"
  echo "zksync_local database created."
}

# Run Celestia node
run_celestia_node() {
  echo "Running Celestia node..."

  docker run --network host -e NODE_TYPE=$NODE_TYPE -e P2P_NETWORK=$NETWORK \
    --name celestia-node \
    -v $(pwd)/../volumes/celestia:/home/celestia \
    ghcr.io/celestiaorg/celestia-node:v0.14.0-rc2 \
    celestia $NODE_TYPE start --core.ip $RPC_URL --p2p.network $NETWORK

  echo "Celestia node is running."
}

# Get node address and show to user
get_node_address() {
  echo "Getting the node address..."
  NODE_ADDRESS=$(docker exec celestia-node celestia state account-address | grep celestia | sed s/'  "result": '//g)
  echo "Node address: $NODE_ADDRESS"
  echo "Please send some TIA tokens in Arabica devnet to the above address to enable it."
  read -p "Press Enter to continue to the next step..."
}

# Extract and export auth token
extract_auth_token() {
  echo "Extracting auth token..."
  CELESTIA_CLIENT_AUTH_TOKEN=$(docker exec celestia-node celestia $NODE_TYPE auth admin --p2p.network $NETWORK)
  export CELESTIA_CLIENT_AUTH_TOKEN
  export NODE_ADDRESS
  echo "Auth token (CELESTIA_CLIENT_AUTH_TOKEN) and node address (NODE_ADDRESS) have been exported to the terminal."
}

# Run database migrations
run_database_migrations() {
  echo "Running database migrations..."
  cargo install sqlx-cli
  export DATABASE_URL=postgres://postgres:notsecurepassword@127.0.0.1:5432/zksync_local
  sqlx migrate run --source ../core/lib/dal/migrations
  echo "Database migrations completed."
}

# Print message about running zksync_server
print_zksync_server_message() {
  echo "In the next step, we are running the zksync_server with only da_dispatcher enabled, and it gets run with the help of node_framework."
  echo "Please wait for the code to build and run. After it is running successfully, run the dummy_data.sh script in another terminal."
  echo "Then return to this terminal to see the result of the da_dispatcher's work that sent the dummy data from the L1 Batch table to Celestia."
}

# Execute the functions
create_directories

# Prompt the user to continue
read -p "Press Enter to continue if Docker is running and the host network feature is enabled..."

check_docker

# Run Docker containers in the background
run_docker_containers &

# Run Celestia node in the background
run_celestia_node &

# Wait for Docker containers and Celestia node to start
# wait

create_zksync_local_db

get_node_address

extract_auth_token

run_database_migrations

# Print message about running zksync_server
print_zksync_server_message

# Run zksync_server with only da_dispatcher enabled
echo "Running zksync_server with only da_dispatcher enabled..."

cargo run --bin zksync_server -- --genesis-path configs/genesis.yaml --wallets-path configs/wallets.yaml --config-path configs/general.yaml --secrets-path configs/secrets.yaml --contracts-config-path configs/contracts.yaml --use-node-framework --components da_dispatcher
