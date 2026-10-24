#!/usr/bin/env bash
# Deploys and wires the Settlix contracts.
#
#   NETWORK=testnet SOURCE=settlix-deployer FEE_BPS=50 ./scripts/deploy.sh
#
# The SOURCE identity becomes admin of all three contracts and, unless
# PAYOUT_WALLET is set, the wallet platform fees are swept to. The native
# XLM asset contract is allow-listed; add more with ALLOW_TOKENS (space
# separated contract IDs), e.g. the network's USDC asset contract.
#
# Contract IDs are written to deployments/$NETWORK.json for the oracle and
# the interface.

set -euo pipefail

NETWORK="${NETWORK:-testnet}"
SOURCE="${SOURCE:-settlix-deployer}"
FEE_BPS="${FEE_BPS:-50}"
ALLOW_TOKENS="${ALLOW_TOKENS:-}"
WASM_DIR="${CARGO_TARGET_DIR:-target}/wasm32v1-none/release"
OUT="deployments/${NETWORK}.json"

cd "$(dirname "$0")/.."

if [[ "$NETWORK" == "mainnet" && "${CONFIRM_MAINNET:-}" != "yes" ]]; then
  echo "Refusing to deploy to mainnet without CONFIRM_MAINNET=yes" >&2
  exit 1
fi

if ! stellar keys address "$SOURCE" >/dev/null 2>&1; then
  if [[ "$NETWORK" == "mainnet" ]]; then
    echo "Identity '$SOURCE' not found. Import it with: stellar keys add $SOURCE" >&2
    exit 1
  fi
  echo "Creating and funding identity '$SOURCE' on $NETWORK"
  stellar keys generate "$SOURCE" --network "$NETWORK" --fund
fi
ADMIN="$(stellar keys address "$SOURCE")"
PAYOUT_WALLET="${PAYOUT_WALLET:-$ADMIN}"
echo "Admin: $ADMIN"

# Always rebuild: a stale wasm would put different code on-chain than this
# checkout, and verify.sh would then report a mismatch.
stellar contract build >/dev/null 2>&1

deploy() {
  stellar contract deploy --wasm "$WASM_DIR/$1.wasm" --source "$SOURCE" --network "$NETWORK"
}

invoke() {
  local id="$1"; shift
  stellar contract invoke --id "$id" --source "$SOURCE" --network "$NETWORK" -- "$@" >/dev/null
}

echo "Deploying contracts"
REGISTRY="$(deploy merchant_registry_contract)"
SETTLEMENT="$(deploy settlement_contract)"
PAYMENT="$(deploy payment_contract)"

echo "Initializing"
invoke "$REGISTRY"   initialize --admin "$ADMIN"
invoke "$SETTLEMENT" initialize --admin "$ADMIN" --payout_wallet "$PAYOUT_WALLET" --fee_bps "$FEE_BPS"
invoke "$PAYMENT"    initialize --admin "$ADMIN" --registry "$REGISTRY" --settlement "$SETTLEMENT"

XLM="$(stellar contract id asset --asset native --network "$NETWORK")"
TOKENS=()
for token in "$XLM" $ALLOW_TOKENS; do
  echo "Allowing token $token"
  invoke "$REGISTRY" set_token_allowed --token "$token" --allowed true
  TOKENS+=("\"$token\"")
done

mkdir -p deployments
cat > "$OUT" <<JSON
{
  "network": "$NETWORK",
  "deployedAt": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "commit": "$(git rev-parse --short HEAD 2>/dev/null || echo unknown)$(git diff --quiet HEAD -- contracts Cargo.toml Cargo.lock 2>/dev/null || echo -dirty)",
  "admin": "$ADMIN",
  "payoutWallet": "$PAYOUT_WALLET",
  "feeBps": $FEE_BPS,
  "contracts": {
    "merchantRegistry": "$REGISTRY",
    "settlement": "$SETTLEMENT",
    "payment": "$PAYMENT"
  },
  "allowedTokens": [$(IFS=,; echo "${TOKENS[*]}")]
}
JSON

echo "Done. Contract IDs written to $OUT"
cat "$OUT"
