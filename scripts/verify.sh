#!/usr/bin/env bash
# Verifies that deployed contracts run exactly the code in this repository.
#
#   NETWORK=testnet ./scripts/verify.sh            # uses deployments/testnet.json
#   NETWORK=testnet ./scripts/verify.sh path.json
#
# Builds every contract, downloads the wasm each deployed contract ID runs,
# and compares SHA-256 hashes. Exits non-zero if any contract differs.

set -euo pipefail

NETWORK="${NETWORK:-testnet}"
MANIFEST="${1:-deployments/${NETWORK}.json}"
WASM_DIR="${CARGO_TARGET_DIR:-target}/wasm32v1-none/release"

cd "$(dirname "$0")/.."

if [[ ! -f "$MANIFEST" ]]; then
  echo "No deployment manifest at $MANIFEST. Deploy first or pass a path." >&2
  exit 2
fi
command -v jq >/dev/null || { echo "jq is required" >&2; exit 2; }

sha() { shasum -a 256 "$1" | cut -d' ' -f1; }

echo "Building contracts from $(git rev-parse --short HEAD)$(git diff --quiet || echo ' (uncommitted changes)')"
stellar contract build >/dev/null 2>&1

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# A function rather than an associative array: macOS ships bash 3.2.
wasm_name() {
  case "$1" in
    merchantRegistry) echo merchant_registry_contract ;;
    settlement) echo settlement_contract ;;
    payment) echo payment_contract ;;
  esac
}

failed=0
printf '%-18s %-8s %s\n' CONTRACT RESULT 'SHA-256 (local)'
for key in merchantRegistry settlement payment; do
  id="$(jq -r ".contracts.${key} // empty" "$MANIFEST")"
  local_wasm="$WASM_DIR/$(wasm_name "$key").wasm"
  if [[ -z "$id" ]]; then
    printf '%-18s %-8s\n' "$key" "missing"
    failed=1
    continue
  fi
  stellar contract fetch --id "$id" --network "$NETWORK" -o "$TMP/$key.wasm" >/dev/null
  if [[ "$(sha "$local_wasm")" == "$(sha "$TMP/$key.wasm")" ]]; then
    printf '%-18s %-8s %s\n' "$key" "match" "$(sha "$local_wasm")"
  else
    printf '%-18s %-8s local %s, on-chain %s\n' "$key" "DIFFERS" "$(sha "$local_wasm")" "$(sha "$TMP/$key.wasm")"
    failed=1
  fi
done

exit "$failed"
