# Settlix Contracts

Stellar Soroban smart contracts for merchant payment and settlement infrastructure.

## Features

- **Payment Processing**: Accept USDC and XLM payments from customers
- **Instant Settlement**: Real-time settlement to merchant wallets
- **Escrow Management**: Secure escrow for disputed transactions
- **Multi-merchant Support**: Single contract serving multiple merchants
- **Fee Collection**: Platform fee collection and distribution

## Contracts

### Payment Contract
- Accept customer payments via QR codes
- Instant settlement to merchant wallets
- Event emission for payment tracking

### Merchant Registry Contract
- Merchant registration and KYC data
- Merchant profile management
- Settlement configuration

### Settlement Contract
- Batch settlement processing
- Cross-border payout support
- Fee distribution logic

## Development

```bash
# Build contracts
cargo build --target wasm32-unknown-unknown --release

# Run tests
cargo test

# Deploy to testnet
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/payment_contract.wasm \
  --source MERCHANT_SECRET \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015"
```

## Project Structure

```
contracts/
├── payment/           # Payment processing contract
├── merchant_registry/ # Merchant management
├── settlement/        # Settlement and payout logic
└── shared/           # Shared types and utilities
```

## Tech Stack

- **Language**: Rust
- **Platform**: Stellar Soroban
- **Testing**: Soroban SDK test utilities
