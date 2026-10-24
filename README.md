# Settlix Contracts

[![CI](https://github.com/SETTLIX/settlix-contracts/actions/workflows/ci.yml/badge.svg)](https://github.com/SETTLIX/settlix-contracts/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![soroban-sdk](https://img.shields.io/badge/soroban--sdk-27.0.6-0b7285)

Soroban smart contracts for Settlix: merchants issue invoices, customers pay
them in USDC or XLM by scanning a QR code, and the merchant is settled in the
same transaction, with the platform fee split off on-chain.

Part of the Settlix stack:
[settlix-contracts](https://github.com/SETTLIX/settlix-contracts) (this repo) ·
[settlix-oracle](https://github.com/SETTLIX/settlix-oracle) (prices and payment verification) ·
[settlix-interface](https://github.com/SETTLIX/settlix-interface) (merchant dashboard and pay page)

## How a payment works

```
 merchant                     payment contract                 customer
    |  create_invoice(25 XLM, "ORDER-1001", expires 1h)            |
    |---------------------------->|                                |
    |        invoice #1 (QR)      |                                |
    |<----------------------------|      pay_invoice(#1)           |
    |                             |<-------------------------------|
    |                             |-- registry.payee(merchant)?    |
    |                             |-- settlement.fee_bps() = 50    |
    |   24.875 XLM to payout addr |                                |
    |<----------------------------|-- 0.125 XLM to settlement vault|
```

One transaction, three contracts, no funds parked in between. If the merchant
was suspended, the token is not allow-listed, the invoice expired, or it was
already paid, nothing moves.

## Contracts

| Crate | Purpose |
| --- | --- |
| [`payment`](contracts/payment/src/lib.rs) | Invoices: create, pay atomically, cancel, refund (partial or full) |
| [`merchant_registry`](contracts/merchant_registry/src/lib.rs) | Merchant applications, admin approval, suspension, settlement address, token allow-list |
| [`settlement`](contracts/settlement/src/lib.rs) | Platform fee vault and the single source of the fee rate; pays out only to the configured wallet |
| [`shared`](contracts/shared/src/lib.rs) | Checked basis-point fee math and storage TTL helpers |

The full function reference, storage layout, events and error codes are in
[docs/architecture.md](docs/architecture.md).

## Quick start

Requirements: Rust stable (the toolchain file adds `wasm32v1-none`) and the
[Stellar CLI](https://developers.stellar.org/docs/tools/cli).

```bash
git clone https://github.com/SETTLIX/settlix-contracts.git
cd settlix-contracts
make check     # rustfmt, clippy -D warnings, 24 tests
make           # optimized wasm for every contract
make sizes
```

## Deploy

```bash
make deploy                       # testnet, funds a fresh deployer identity
NETWORK=testnet ./scripts/verify.sh   # on-chain wasm == this source?
```

`deploy.sh` deploys all three contracts, initializes them with the deployer
as admin, allow-lists native XLM, and writes the contract IDs to
`deployments/<network>.json`. The current testnet deployment is in
[deployments/testnet.json](deployments/testnet.json). See
[docs/deployment.md](docs/deployment.md) for mainnet, USDC and key rotation.

## Using the contracts from the CLI

```bash
PAY=$(jq -r .contracts.payment deployments/testnet.json)

# merchant opens an invoice for 25 XLM (7 decimals), payable for an hour
stellar contract invoke --id $PAY --source merchant --network testnet -- \
  create_invoice --merchant <G...> --token <XLM SAC> --amount 250000000 \
  --reference '"ORDER-1001"' --expires_at $(( $(date +%s) + 3600 ))

# customer pays it
stellar contract invoke --id $PAY --source customer --network testnet -- \
  pay_invoice --customer <G...> --id 1
```

## Security

The contracts have not been externally audited. The security properties we
rely on, and how to report a vulnerability privately, are in
[SECURITY.md](SECURITY.md).

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) and the
[Code of Conduct](CODE_OF_CONDUCT.md). Contributors are listed in
[CONTRIBUTORS.md](CONTRIBUTORS.md).

## License

[MIT](LICENSE)
