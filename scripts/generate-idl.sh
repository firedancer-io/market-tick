#!/bin/sh
set -eu

root_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
output_dir="$root_dir/idls"
output_file="$output_dir/market_tick.json"
temporary_file="$output_dir/.market_tick.json.tmp"

mkdir -p "$output_dir"
trap 'rm -f "$temporary_file"' EXIT HUP INT TERM

cd "$root_dir"
anchor idl build --program-name market-tick --out "$temporary_file"

# Only replace the committed artifact after generation succeeds.
mv "$temporary_file" "$output_file"
trap - EXIT HUP INT TERM

echo "Generated $output_file"
