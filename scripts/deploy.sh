#!/bin/sh
set -eu

# Prepare a deployment from the repository root:
#
#   1. Securely provide the program address keypair whose public key is
#      `tickUcsEQegChaAuo9VYQQztB4ZGApY6ZT4FkULWY6N`.
#
#   2. Commit and push the release to the public repository configured
#      in Cargo.toml. The working tree must be clean. This script runs
#      `solana-verify build --library-name market_tick` and deploys the
#      resulting target/deploy/market_tick.so from that exact commit.
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
  --expected-genesis-hash HASH        Required with --rpc-url; not valid with
                                      -um, -ut, or -ud
  --skip-onchain-metadata             Skip Anchor IDL and verified-build metadata
                                      uploads; executable hash checks still run
  --yes                               Skip the deployment confirmation prompt
EOF
    exit 2
}

# Match program-metadata client version pinned by anchor-cli 1.1.2
PMP_CLIENT_VERSION=0.5.1

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
        --rpc-url|--expected-genesis-hash|--fee-payer|--program-keypair|--squads-vault-pubkey|--authority-keypair)
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

for tool in anchor curl git jq node npx solana solana-keygen solana-verify; do
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

root_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
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
source_pda=$(sed -n 's/.*const PDA_V1: Pubkey = pubkey!("\([^"]*\)").*/\1/p' "$source_file")
derived_pda=$(solana find-program-derived-address "$program_id" \
    string:market_tick string:v1 --output json-compact | jq -er '.address')
[ -n "$source_pda" ] && [ "$source_pda" = "$derived_pda" ] || {
    echo "compiled V1 PDA does not match the program ID and V1 seeds" >&2
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
    solana-verify build --library-name market_tick
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

existing_authority=
if program_json=$(solana program show --url "$rpc_url" --output json "$program_id" 2>/dev/null); then
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

if [ "$auto_confirm" -eq 0 ]; then
    printf 'Deploy this release? [y/N] '
    read answer
    case "$answer" in y|Y|yes|YES) ;; *) echo "aborted"; exit 1 ;; esac
fi

solana program deploy --url "$rpc_url" --commitment finalized --use-rpc \
    --max-sign-attempts 100 \
    --fee-payer "$fee_payer" \
    --keypair "$deployment_authority_keypair" \
    --upgrade-authority "$deployment_authority_keypair" \
    --program-id "$program_keypair" \
    "$program_binary"

local_hash=$(solana-verify get-executable-hash "$program_binary" | awk 'NF { value=$NF } END { print value }')
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

    solana-verify --url "$rpc_url" verify-from-repo \
        --program-id "$program_id" \
        --commit-hash "$commit" \
        --library-name market_tick \
        --keypair "$deployment_authority_keypair" \
        --skip-prompt \
        "$repository"
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
