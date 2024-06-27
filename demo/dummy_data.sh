#!/bin/bash

# Dummy Data Script

# Define colors
GREEN='\033[0;32m'
NC='\033[0m' # No Color

# Exit immediately if a command exits with a non-zero status.
set -e

export DATABASE_URL=postgres://postgres:notsecurepassword@127.0.0.1:5432/zksync_local

# Change to the demo directory
cd "$(dirname "$0")"

# Run the cargo run command
echo "Running cargo run to execute the dummy data script..."
cargo run
echo -e "${GREEN}Dummy data script executed successfully.${NC}"