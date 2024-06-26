#!/bin/bash

# Dummy Data Script

# Exit immediately if a command exits with a non-zero status.
set -e

# Change to the demo directory
cd "$(dirname "$0")"

# Run the cargo run command
echo "Running cargo run to execute the dummy data script..."
cargo run

echo "Dummy data script executed successfully."