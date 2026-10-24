# Deployment

## Testnet

```bash
make deploy
NETWORK=testnet ./scripts/verify.sh
```

`scripts/deploy.sh`:

1. Creates and funds the `settlix-deployer` identity with Friendbot if it
   does not exist (`SOURCE=` to use another).
2. Rebuilds every contract, so the wasm on-chain is always this checkout.
3. Deploys `merchant_registry`, `settlement` and `payment`.
4. Initializes them. The deployer is admin of all three, and also the payout
   wallet unless `PAYOUT_WALLET=G...` is set. The fee is `FEE_BPS` (default
   50, that is 0.5%).
5. Allow-lists the native XLM asset contract, plus anything in
   `ALLOW_TOKENS="C... C..."`.
6. Writes `deployments/<network>.json`, which the oracle and the interface
   read.

Then onboard a merchant:

```bash
REG=$(jq -r .contracts.merchantRegistry deployments/testnet.json)
HASH=$(printf 'RC-123456' | shasum -a 256 | cut -d' ' -f1)

stellar contract invoke --id $REG --source merchant --network testnet -- \
  apply --merchant <merchant G...> --name '"Ada Fabrics"' \
  --business_hash $HASH --settlement_address <payout G...>

stellar contract invoke --id $REG --source settlix-deployer --network testnet -- \
  approve --merchant <merchant G...>
```

## USDC

Find the asset contract for the USDC issuer on your network and allow it:

```bash
USDC=$(stellar contract id asset --network testnet \
  --asset USDC:GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5)
ALLOW_TOKENS=$USDC make deploy
# or, on an existing deployment:
stellar contract invoke --id $REG --source settlix-deployer --network testnet -- \
  set_token_allowed --token $USDC --allowed true
```

Customers and merchant payout accounts need a USDC trustline before they can
hold it.

## Mainnet

`deploy.sh` refuses mainnet unless `CONFIRM_MAINNET=yes` is set, and will not
generate a key there: import one with `stellar keys add`. Before deploying
with real value:

- Get the contracts audited (see [SECURITY.md](../SECURITY.md)).
- Use a hardware or multisig account as admin, then `transfer_admin` to it
  on all three contracts.
- Point the payout wallet at the platform's treasury account with
  `set_payout_wallet` (the wallet must co-sign).
- Run `verify.sh` against the mainnet manifest and publish the result.

## Verifying a deployment

`scripts/verify.sh` rebuilds the contracts, fetches the wasm each deployed ID
runs, and compares SHA-256 hashes. Anyone can run it to confirm the deployed
code is the code in this repository at a given commit. Each wasm also carries
`source_repo` metadata (SEP-0055):

```bash
stellar contract info meta --network testnet --contract-id <contract id>
```
