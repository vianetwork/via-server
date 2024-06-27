#!/usr/bin/env bash

# Demo Script for Project Feature

# Define colors
RED='\033[0;31m'
GREEN='\033[0;32m'
GREEN_BOLD='\033[1;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# Exit immediately if a command exits with a non-zero status.
set -e

export NETWORK=arabica
export NODE_TYPE=light
export RPC_URL=validator-2.celestia-arabica-11.com
export CELESTIA_CLIENT_API_NODE_URL=ws://localhost:26658
export CELESTIA_CLIENT_API_PRIVATE_KEY=""
export NODE_STORE=$HOME/.celestia-light-arabica-11
export DATABASE_URL=postgres://postgres:notsecurepassword@127.0.0.1:5432/zksync_local

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
done

# Check if VIA_HOME environment variable is set
check_via_home() {
  if [[ -z "${VIA_HOME}" ]]; then
    echo -e "${RED}Environment variable VIA_HOME is not set. Make sure it's set and pointing to the root of this repository.${NC}"
    exit 1
  fi
}

# wait until a container is running
# $1 is the container name
wait_for() {
  while [ $(docker inspect -f {{.State.Running}} $1) = "false" ]; do
    sleep 0.1;
  done
}

# Create directories for volumes
create_directories() {
  echo "Creating directories for volumes..."
  mkdir -p ./volumes/{postgres,celestia,reth/data}
  echo -e "${GREEN}Volume directories created.${NC}"
}

# Check if Docker is running
check_docker() {
  echo "Checking if Docker is running and the host network feature is enabled..."
  docker info > /dev/null 2>&1
  if [ $? -ne 0 ]; then
    echo -e "${RED}Docker does not seem to be running. Please start Docker and ensure the host network feature is enabled.${NC}"
    exit 1
  fi
}

# Run Docker containers in detached mode
run_docker_containers() {
  echo "Running reth, postgres and celestia-node services in detached mode..."
  docker-compose -f docker-compose-celestia-demo.yml up -d &
  echo -e "${GREEN}reth, postgres and celestia-node services are now running in detached mode.${NC}"
}

# Create zksync_local database
create_zksync_local_db() {
  echo "Creating zksync_local database..."
  # postgres does not support IF EXISTS statement for create database,
  # so we have to do this ugly montruosity to avoid an error.
  # this checks if the database already exists.
  if [ "$(docker exec -i "$(docker ps -q -f name=postgres)" psql -XtA -h 127.0.0.1 -p 5432 -U postgres -d postgres -c "SELECT 1 FROM pg_database WHERE datname='zksync_local';")" = '1' ]
  then
    echo -e "${YELLOW}Database already exists, ignoring${NC}"
  else
    docker exec -i "$(docker ps -q -f name=postgres)" psql -h 127.0.0.1 -p 5432 -U postgres -d postgres -c "CREATE DATABASE zksync_local;" <<< "notsecurepassword"
    echo -e "${GREEN_BOLD}zksync_local${GREEN} database created.${NC} ✔"
  fi
}

# Get and export Celestia node address
get_node_address() {
  echo "Getting the node address..."
  NODE_ADDRESS=$(docker exec $(docker ps -q -f name=celestia-node) celestia state account-address | grep celestia | sed s/'  "result": '//g)
  export NODE_ADDRESS
  echo -e "${GREEN}Celestia Node Address: $NODE_ADDRESS${NC}"
  echo "------ ⚠️Important! ------"
  echo -e "${YELLOW}Please send some TIA tokens in Arabica devnet to the above address to enable it."
  echo -e "Check Arabica devnet faucet documentation: https://docs.celestia.org/nodes/arabica-devnet#arabica-devnet-faucet,"
  echo -e "or follow these steps to transfer tokens from another account https://docs.celestia.org/developers/node-tutorial#transfer-balance-of-utia-to-another-account.${NC}"
  read -r -p "Press Enter to continue to the next step..."
}

# Extract and export auth token
extract_auth_token() {
  echo "Extracting auth token..."
  CELESTIA_CLIENT_AUTH_TOKEN=$(docker exec $(docker ps -q -f name=celestia-node) celestia $NODE_TYPE auth admin --p2p.network $NETWORK)
  export CELESTIA_CLIENT_AUTH_TOKEN
  echo -e "${GREEN}Celestia Client Auth Token: $CELESTIA_CLIENT_AUTH_TOKEN${NC}"
}

run_database_migrations() {
  echo "Running database migrations..."
  cargo install sqlx-cli --version 0.7.3
  sqlx migrate run --source ./core/lib/dal/migrations
  echo -e "${GREEN}Database migrations completed.${NC}"
}

# Print message about running zksync_server
print_via_server_message() {
  echo -e "${YELLOW}In the next step, we are running the zksync_server with only da_dispatcher enabled, and it gets run with the help of node_framework."
  echo -e "Please wait for the code to build and run. After it is running successfully, run the dummy_data.sh script in another terminal."
  echo -e "Then return to this terminal to see the result of the da_dispatcher's work that sent the dummy data from the L1 Batch table to Celestia.${NC}"
}

# Run zksync_server with only da_dispatcher enabled
run_via_server() {
  echo "Running zksync_server with only da_dispatcher enabled..."

  cargo run --bin zksync_server -- \
    --genesis-path ./demo/configs/genesis.yaml \
    --wallets-path ./demo/configs/wallets.yaml \
    --config-path ./demo/configs/general.yaml \
    --secrets-path ./demo/configs/secrets.yaml \
    --contracts-config-path ./demo/configs/contracts.yaml \
    --use-node-framework \
    --components da_dispatcher
}

check_via_home
cd $VIA_HOME

create_directories

# Prompt the user to continue
read -r -p "Press Enter to continue if Docker is running and the host network feature is enabled..."

check_docker

# Run Docker containers in the background
run_docker_containers

# # Wait for Docker containers to start
# wait_for via-server-celestia-node-1
# wait_for via-server-reth-1
# wait_for via-server-postgres-1
sleep 1

create_zksync_local_db

get_node_address

extract_auth_token

run_database_migrations

print_via_server_message

run_via_server
