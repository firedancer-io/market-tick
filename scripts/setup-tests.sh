#!/bin/sh
set -eu

root_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
source_keypair="$root_dir/market-tick/tests/fixtures/keys/local.json"
target_dir="$root_dir/target/deploy"
target_keypair="$target_dir/market_tick-keypair.json"

mkdir -p "$target_dir"

cp "$source_keypair" "$target_keypair"
chmod 600 "$target_keypair"

program_id=$(solana-keygen pubkey "$target_keypair")
echo "Local market-tick program ID: $program_id"
