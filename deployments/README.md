# Deployments

One JSON manifest per network, written by `scripts/deploy.sh`:

| Field | Meaning |
| --- | --- |
| `contracts.merchantRegistry`, `contracts.settlement`, `contracts.payment` | Contract IDs |
| `admin` | Admin of all three contracts at deploy time |
| `payoutWallet` | Where swept fees go |
| `feeBps` | Fee rate at deploy time (read the live value with `settlement.fee_bps`) |
| `allowedTokens` | Asset contracts allow-listed at deploy time |
| `commit` | Source commit the wasm was built from |

Check that a manifest matches this source with `./scripts/verify.sh`.
