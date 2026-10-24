# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `payment`: invoices. A merchant creates an invoice with its own reference
  and an expiry; a customer pays it in one call that checks the registry,
  takes the fee and pays the merchant atomically. Cancellation and partial or
  full refunds by the payee.
- `merchant_registry`: application and approval flow (`Pending`, `Active`,
  `Suspended`), SHA-256 business identifier instead of raw business data,
  merchant-controlled settlement address, and a token allow-list.
- `settlement`: platform fee vault. Holds the fee rate the payment contract
  reads, and pays out only to the configured wallet.
- `shared` crate: checked fee math and TTL helpers.
- Typed `#[contracterror]` enums and `#[contractevent]` events everywhere.
- 24 unit and cross-contract tests, with committed snapshots.
- `scripts/deploy.sh` and `scripts/verify.sh`, a Makefile, CI (fmt, clippy,
  tests, wasm build), and a testnet deployment in `deployments/`.
- Security policy, contributing guide, code of conduct, issue forms.

### Fixed

- `settlement.execute_settlement` had no authorization and sent fees to a
  wallet chosen by the caller, so anyone could drain collected fees.
- `initialize` on every contract had no authorization and could be
  front-run by anyone after deployment.
- Payments never checked the merchant registry: unregistered or suspended
  merchants could be paid.
- Payment IDs and amounts were not tied to an order, so the same order could
  be paid twice.
- Panics replaced by typed errors that clients can handle.

### Changed

- `soroban-sdk` 22 to 27.0.6; `Cargo.lock` is now committed for reproducible
  builds.

### Removed

- Generated `.meta/commits` filler files.
