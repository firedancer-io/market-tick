#!/bin/sh
set -eu

# Release configuration. Keep these in sync with the Cargo/Anchor pins.
ANCHOR_VERSION=1.2.0
SOLANA_VERSION=4.3.0
SOLANA_VERIFY_VERSION=0.5.2
BUILD_SBF_VERSION=4.3.0
# Match the program-metadata client used by the selected Anchor release.
PMP_CLIENT_VERSION=0.5.1
MARKET_TICK_ARCH=v3
MARKET_TICK_TOOLS_VERSION=v1.57
MARKET_TICK_ELF_MACHINE=0xF7 # EM_BPF
MARKET_TICK_ACCOUNT_VERSION=1
# Upstream image selected by solana-verify for the Cargo.toml Solana pin.
BUILD_IMAGE=solanafoundation/solana-verifiable-build@sha256:12fd4c0a0790f0fc41ef74b0cdb6bccc167ba6137eb1adb749bac649481c86bd

# Prepare a deployment from the repository root:
#
#   1. Securely provide the program address keypair whose public key is
#      `tickUcsEQegChaAuo9VYQQztB4ZGApY6ZT4FkULWY6N`.
#
#   2. Commit and push the release to the public repository configured
#      in Cargo.toml. The working tree must be clean. This script runs
#      `solana-verify build`, selecting the upstream image from the
#      Solana version pin in Cargo.toml, and deploys the resulting
#      target/deploy/market_tick.so from that exact commit.
#
#   3. Fund the --fee-payer and check its balance:
#        solana balance --url <RPC_URL> /secure/path/fee-payer.json
#
#   4. Select the upgrade authority:
#        - Pass --authority-keypair for a single authority; or
#        - Pass the verified Squads vault PDA as --squads-vault-pubkey.
#
# The script generates and uploads the Anchor IDL, uploads Solana
# verified-build metadata for the current commit, and then transfers
# authority when using Squads. Verify the Squads vault separately before
# deployment and verify all on-chain state afterward.
usage() {
    cat >&2 <<EOF
Usage:
  $0 CLUSTER COMMON_OPTIONS AUTHORITY [--yes]

Cluster (choose one):
  -um                                 Use Solana mainnet-beta
  -ut                                 Use Solana testnet
  -ud                                 Use Solana devnet
  --rpc-url URL                       Use a custom RPC endpoint; requires
                                      --expected-genesis-hash

Common options (all required):
  --fee-payer PATH                    Transaction fee-payer keypair
  --program-keypair PATH              Program address keypair

Authority (choose one mode):
  --authority-keypair PATH            Keep this keypair as upgrade authority
  OR
  --squads-vault-pubkey PUBKEY        Use this Squads vault as upgrade authority

Other options:
  --buffer-keypair PATH               Retain this buffer keypair for retries
  --expected-genesis-hash HASH        Required with --rpc-url; not valid with
                                      -um, -ut, or -ud
  --skip-onchain-metadata             Skip Anchor IDL and verified-build metadata
                                      uploads; executable hash checks still run
  --prepare-only                      Build and check the release without signing
  --yes                               Skip the deployment confirmation prompt
EOF
    exit 2
}

rpc_url=
expected_genesis_hash=
expected_genesis_hash_supplied=0
cluster_selected=0
cluster_shortcut=0
fee_payer=
program_keypair=
vault_pubkey=
authority_keypair=
auto_confirm=0
skip_onchain_metadata=0
prepare_only=0
buffer_keypair=

while [ "$#" -gt 0 ]; do
    case "$1" in
        -um|-ut|-ud)
            [ "$cluster_selected" -eq 0 ] && \
                [ "$expected_genesis_hash_supplied" -eq 0 ] || usage
            cluster_selected=1
            cluster_shortcut=1
            case "$1" in
                -um)
                    rpc_url=https://api.mainnet-beta.solana.com
                    expected_genesis_hash=5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d
                    ;;
                -ut)
                    rpc_url=https://api.testnet.solana.com
                    expected_genesis_hash=4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY
                    ;;
                -ud)
                    rpc_url=https://api.devnet.solana.com
                    expected_genesis_hash=EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG
                    ;;
            esac
            shift
            ;;
        --rpc-url|--expected-genesis-hash|--fee-payer|--program-keypair|--squads-vault-pubkey|--authority-keypair|--buffer-keypair)
            [ "$#" -ge 2 ] || usage
            case "$1" in
                --rpc-url)
                    [ "$cluster_selected" -eq 0 ] || usage
                    cluster_selected=1
                    rpc_url=$2
                    ;;
                --expected-genesis-hash)
                    [ "$cluster_shortcut" -eq 0 ] && \
                        [ "$expected_genesis_hash_supplied" -eq 0 ] || usage
                    expected_genesis_hash=$2
                    expected_genesis_hash_supplied=1
                    ;;
                --fee-payer) fee_payer=$2 ;;
                --program-keypair) program_keypair=$2 ;;
                --squads-vault-pubkey) vault_pubkey=$2 ;;
                --authority-keypair) authority_keypair=$2 ;;
                --buffer-keypair) buffer_keypair=$2 ;;
            esac
            shift 2
            ;;
        --skip-onchain-metadata)
            skip_onchain_metadata=1
            shift
            ;;
        --yes)
            auto_confirm=1
            shift
            ;;
        --prepare-only)
            prepare_only=1
            shift
            ;;
        *) usage ;;
    esac
done

[ "$cluster_selected" -eq 1 ] && [ -n "$rpc_url" ] && \
    [ -n "$expected_genesis_hash" ] && [ -n "$fee_payer" ] && \
    [ -n "$program_keypair" ] || usage

if [ -n "$vault_pubkey" ]; then
    [ -z "$authority_keypair" ] || usage
    authority_mode="Squads vault"
    deployment_authority_keypair=$fee_payer
    final_authority=$vault_pubkey
else
    [ -n "$authority_keypair" ] || usage
    authority_mode="keypair"
    deployment_authority_keypair=$authority_keypair
fi

for tool in anchor curl docker git jq node npx python3 solana solana-keygen solana-verify; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "missing required tool: $tool" >&2
        exit 1
    }
done

[ -f "$fee_payer" ] || { echo "missing fee-payer keypair: $fee_payer" >&2; exit 1; }
[ -f "$program_keypair" ] || { echo "missing program keypair: $program_keypair" >&2; exit 1; }
[ -f "$deployment_authority_keypair" ] || {
    echo "missing authority keypair: $deployment_authority_keypair" >&2
    exit 1
}
# Source verification changes directory; preserve relative keypair arguments.
absolute_keypair_path() {
    printf '%s/%s\n' "$(CDPATH= cd -- "$(dirname -- "$1")" && pwd)" "$(basename -- "$1")"
}
fee_payer=$(absolute_keypair_path "$fee_payer")
program_keypair=$(absolute_keypair_path "$program_keypair")
deployment_authority_keypair=$(absolute_keypair_path "$deployment_authority_keypair")
if [ -n "$buffer_keypair" ]; then
    [ -f "$buffer_keypair" ] || { echo "missing buffer keypair: $buffer_keypair" >&2; exit 1; }
    buffer_keypair=$(absolute_keypair_path "$buffer_keypair")
fi

root_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
check_tool_version() {
    tool_version=$("$1" --version | awk '{print $2}')
    [ "$tool_version" = "$2" ] || {
        echo "$1 $2 required (found ${tool_version:-unknown})" >&2
        exit 1
    }
}
check_tool_version anchor "$ANCHOR_VERSION"
check_tool_version solana "$SOLANA_VERSION"
check_tool_version solana-verify "$SOLANA_VERIFY_VERSION"
[ -z "$(git -C "$root_dir" status --porcelain)" ] || {
    echo "deployment requires a clean Git working tree" >&2
    exit 1
}
commit=$(git -C "$root_dir" rev-parse HEAD)
repository=$(sed -n 's/^repository = "\([^"]*\)"/\1/p' "$root_dir/Cargo.toml")
[ -n "$repository" ] || { echo "missing repository URL in Cargo.toml" >&2; exit 1; }
case "$repository" in
    https://github.com/*) ;;
    *) echo "repository must be a public GitHub HTTPS URL: $repository" >&2; exit 1 ;;
esac
repository=${repository%.git}
commit_url="$repository/commit/$commit"
curl -qfsSIL --output /dev/null "$commit_url" || {
    echo "commit is not publicly available on GitHub: $commit_url" >&2
    echo "push the commit before deploying" >&2
    exit 1
}

program_id=$(solana-keygen pubkey "$program_keypair")
source_file="$root_dir/programs/market-tick/src/lib.rs"
source_program_id=$(sed -n 's/.*declare_id!("\([^"]*\)").*/\1/p' "$source_file")
[ "$program_id" = "$source_program_id" ] || {
    echo "program keypair does not match declare_id!()" >&2
    exit 1
}
source_pda=$(sed -n 's/.*const PDA_V'"$MARKET_TICK_ACCOUNT_VERSION"': Pubkey = pubkey!("\([^"]*\)").*/\1/p' "$source_file")
derived_pda=$(solana find-program-derived-address "$program_id" \
    string:market_tick "string:v$MARKET_TICK_ACCOUNT_VERSION" --output json-compact | jq -er '.address')
[ -n "$source_pda" ] && [ "$source_pda" = "$derived_pda" ] || {
    echo "compiled V$MARKET_TICK_ACCOUNT_VERSION PDA does not match the program ID and seeds" >&2
    echo "compiled: ${source_pda:-missing}" >&2
    echo "derived:  $derived_pda" >&2
    exit 1
}

committed_idl="$root_dir/idls/market_tick.json"
[ -f "$committed_idl" ] || {
    echo "missing committed IDL: $committed_idl" >&2
    exit 1
}
idl_file=$(mktemp "${TMPDIR:-/tmp}/market-tick-idl.XXXXXX.json")
onchain_idl_file=$(mktemp "${TMPDIR:-/tmp}/market-tick-onchain-idl.XXXXXX.json")
trap 'rm -f "$idl_file" "$onchain_idl_file"' EXIT HUP INT TERM
(
    cd "$root_dir"
    anchor idl build --program-name market-tick --out "$idl_file"
    image_versions=$(docker run --rm "$BUILD_IMAGE" cargo build-sbf --version)
    printf '%s\n' "$image_versions" | grep -Fxq "cargo-build-sbf $BUILD_SBF_VERSION" && \
        printf '%s\n' "$image_versions" | grep -Fxq "platform-tools $MARKET_TICK_TOOLS_VERSION" || {
        echo "build image must contain cargo-build-sbf $BUILD_SBF_VERSION and platform-tools $MARKET_TICK_TOOLS_VERSION" >&2
        exit 1
    }
    mkdir -p target
    build_log=$(mktemp "$root_dir/target/sbpf-build.XXXXXX.log")
    if ! solana-verify build --library-name market_tick \
        --arch "$MARKET_TICK_ARCH" \
        --cargo-build-sbf-args="--tools-version $MARKET_TICK_TOOLS_VERSION" >"$build_log" 2>&1; then
        cat "$build_log"
        exit 1
    fi
    cat "$build_log"
    # LLVM stack diagnostics do not necessarily make Cargo fail.
    if grep -Ei 'stack offset.*exceeded|stack frame.*exceeded|stack size.*exceeded|Error:' "$build_log"; then
        echo "release build has compiler errors; see $build_log" >&2
        exit 1
    fi
)
program_binary="$root_dir/target/deploy/market_tick.so"
[ -f "$program_binary" ] || {
    echo "verifiable build did not produce $program_binary" >&2
    exit 1
}
idl_program_id=$(jq -er '.address | select(type == "string" and length > 0)' "$idl_file")
[ "$idl_program_id" = "$program_id" ] || {
    echo "generated IDL address does not match the program keypair" >&2
    exit 1
}
committed_idl_json=$(jq -S -c . "$committed_idl")
generated_idl_json=$(jq -S -c . "$idl_file")
[ "$committed_idl_json" = "$generated_idl_json" ] || {
    echo "generated IDL does not match the committed IDL" >&2
    echo "run scripts/generate-idl.sh, review the changes, and commit them" >&2
    exit 1
}

actual_genesis_hash=$(solana genesis-hash --url "$rpc_url")
[ "$actual_genesis_hash" = "$expected_genesis_hash" ] || {
    echo "unexpected cluster genesis hash" >&2
    echo "expected: $expected_genesis_hash" >&2
    echo "actual:   $actual_genesis_hash" >&2
    exit 1
}
fee_payer_pubkey=$(solana-keygen pubkey "$fee_payer")
deployment_authority=$(solana-keygen pubkey "$deployment_authority_keypair")

if [ -z "$vault_pubkey" ]; then
    final_authority=$deployment_authority
fi
python3 - "$program_binary" "$MARKET_TICK_ARCH" "$MARKET_TICK_ELF_MACHINE" <<'PY'
import struct
import sys

# ELF e_flags stores the numeric sBPF architecture version.
expected_version = int(sys.argv[2].removeprefix('v'))
expected_machine = int(sys.argv[3], 0)
with open(sys.argv[1], 'rb') as program:
    header = program.read(64)
if (len(header) != 64 or header[:7] != b'\x7fELF\x02\x01\x01'
        or struct.unpack_from('<H', header, 18)[0] != expected_machine
        or struct.unpack_from('<I', header, 48)[0] != expected_version):
    sys.exit(f'release artifact must be little-endian ELF64 / machine={expected_machine:#x} / SBPFv{expected_version}')
PY

existing_authority=
if program_json=$(solana program show --url "$rpc_url" \
    --keypair "$deployment_authority_keypair" --output json "$program_id" 2>/dev/null); then
    existing_authority=$(printf '%s' "$program_json" | jq -r '.authority // .upgradeAuthority // empty')
    if [ "$existing_authority" = "$final_authority" ] && [ -n "$vault_pubkey" ]; then
        echo "program is already controlled by the Squads vault; use the multisig upgrade flow" >&2
        exit 1
    fi
    [ "$existing_authority" = "$deployment_authority" ] || {
        echo "program has unexpected upgrade authority: ${existing_authority:-none}" >&2
        exit 1
    }
fi

balance=$(solana balance --url "$rpc_url" --lamports "$fee_payer_pubkey" | awk '{print $1}')
[ "$balance" -gt 0 ] || { echo "fee payer is not funded" >&2; exit 1; }

echo "RPC URL: $rpc_url"
echo "Genesis hash: $actual_genesis_hash"
echo "Program ID: $program_id"
echo "Source commit: $commit"
echo "Repository: $repository"
echo "Program binary: $program_binary"
echo "Build image: $BUILD_IMAGE"
local_hash=$(solana-verify get-executable-hash "$program_binary" | awk 'NF { value=$NF } END { print value }')
[ -n "$local_hash" ] || { echo 'missing executable hash' >&2; exit 1; }
echo "Program hash: $local_hash"
echo "Generated IDL: $idl_file"
echo "Authority mode: $authority_mode"
if [ "$skip_onchain_metadata" -eq 1 ]; then
    echo "On-chain metadata: skipped"
else
    echo "On-chain metadata: enabled"
fi
if [ -n "$vault_pubkey" ]; then
    echo "Squads vault: $vault_pubkey"
fi
echo "Final upgrade authority: $final_authority"
if [ "$prepare_only" -eq 1 ]; then
    echo 'Preparation complete; no transactions submitted'
    exit 0
fi

if [ "$auto_confirm" -eq 0 ]; then
    printf 'Deploy this release? [y/N] '
    read answer
    case "$answer" in y|Y|yes|YES) ;; *) echo "aborted"; exit 1 ;; esac
fi

set --
if [ -n "$buffer_keypair" ]; then
    set -- --buffer "$buffer_keypair"
fi
solana program deploy --url "$rpc_url" --commitment finalized --use-rpc \
    --max-sign-attempts 100 \
    --fee-payer "$fee_payer" \
    --keypair "$deployment_authority_keypair" \
    --upgrade-authority "$deployment_authority_keypair" \
    --program-id "$program_keypair" \
    "$@" \
    "$program_binary"

onchain_hash=$(solana-verify --url "$rpc_url" get-program-hash "$program_id" | awk 'NF { value=$NF } END { print value }')
[ -n "$local_hash" ] && [ "$local_hash" = "$onchain_hash" ] || {
    echo "deployed program does not match the verifiable build" >&2
    echo "local:    ${local_hash:-unknown}" >&2
    echo "on-chain: ${onchain_hash:-unknown}" >&2
    exit 1
}

if [ "$skip_onchain_metadata" -eq 0 ]; then
    if anchor idl fetch --provider.cluster "$rpc_url" "$program_id" >/dev/null 2>&1; then
        anchor idl upgrade --provider.cluster "$rpc_url" \
            --provider.wallet "$deployment_authority_keypair" \
            --commitment finalized --filepath "$idl_file" "$program_id"
    else
        anchor idl init --provider.cluster "$rpc_url" \
            --provider.wallet "$deployment_authority_keypair" \
            --commitment finalized --filepath "$idl_file" "$program_id"
    fi
    anchor idl fetch --provider.cluster "$rpc_url" --commitment finalized \
        --out "$onchain_idl_file" "$program_id"
    onchain_idl_json=$(jq -S -c . "$onchain_idl_file")
    [ "$onchain_idl_json" = "$generated_idl_json" ] || {
        echo "on-chain IDL does not match the generated and committed IDL" >&2
        exit 1
    }

    # Keep the verification clone under the mounted workspace, so sibling
    # Docker builds can read it when this script runs inside a container.
    mkdir -p "$root_dir/target/verification"
    (
        cd "$root_dir/target/verification"
        solana-verify --url "$rpc_url" verify-from-repo --current-dir \
            --program-id "$program_id" \
            --commit-hash "$commit" \
            --library-name market_tick \
            --arch "$MARKET_TICK_ARCH" \
            --cargo-build-sbf-args="--tools-version $MARKET_TICK_TOOLS_VERSION" \
            --keypair "$deployment_authority_keypair" \
            --skip-prompt \
            "$repository"
    )
else
    echo "Skipping Anchor IDL and verified-build metadata uploads"
fi

if [ -n "$vault_pubkey" ]; then
    if [ "$skip_onchain_metadata" -eq 0 ]; then
        npx --yes "--package=@solana-program/program-metadata@$PMP_CLIENT_VERSION" -- \
            program-metadata remove-authority idl "$program_id" \
            --rpc "$rpc_url" \
            --keypair "$deployment_authority_keypair"
    fi

    solana program set-upgrade-authority --url "$rpc_url" \
        --commitment finalized \
        --fee-payer "$fee_payer" \
        --keypair "$deployment_authority_keypair" \
        --upgrade-authority "$deployment_authority_keypair" \
        --new-upgrade-authority "$final_authority" \
        --skip-new-upgrade-authority-signer-check \
        "$program_id"
fi

final_program_json=$(solana program show --url "$rpc_url" \
    --keypair "$deployment_authority_keypair" \
    --commitment finalized --output json "$program_id")
observed_authority=$(printf '%s' "$final_program_json" | \
    jq -r '.authority // .upgradeAuthority // empty')
[ "$observed_authority" = "$final_authority" ] || {
    echo "final upgrade authority mismatch" >&2
    echo "expected: $final_authority" >&2
    echo "actual:   ${observed_authority:-none}" >&2
    exit 1
}

final_program_hash=$(solana-verify --url "$rpc_url" \
    get-program-hash "$program_id" | awk 'NF { value=$NF } END { print value }')
[ -n "$final_program_hash" ] && [ "$final_program_hash" = "$local_hash" ] || {
    echo "final program hash mismatch" >&2
    echo "expected: $local_hash" >&2
    echo "actual:   ${final_program_hash:-unknown}" >&2
    exit 1
}

echo "Deployment completed successfully"
echo "Program ID: $program_id"
echo "Program hash: $final_program_hash"
if [ "$skip_onchain_metadata" -eq 0 ]; then
    echo "IDL uploaded from commit: $commit"
    echo "Verified-build metadata uploaded for: $repository"
else
    echo "On-chain IDL and verified-build metadata: skipped"
    echo "Committed IDL validated at: $committed_idl"
fi
echo "Upgrade authority: $observed_authority"
