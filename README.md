# Settlix Contracts

<div align="center">

![Stellar](https://img.shields.io/badge/Stellar-Soroban-blue)
![Rust](https://img.shields.io/badge/Rust-1.70+-orange)
![License](https://img.shields.io/badge/license-MIT-green)

Stellar Soroban smart contracts for merchant payment and settlement infrastructure.

**[Features](#features) • [Contracts](#contracts) • [Installation](#installation) • [Usage](#usage) • [Documentation](#documentation)**

</div>

---

## 🌟 Features

- ✅ **Payment Processing**: Accept USDC and XLM payments from customers via QR codes
- ⚡ **Instant Settlement**: Real-time settlement to merchant wallets with automatic fee calculation
- 🏪 **Merchant Registry**: Comprehensive merchant management with KYC support
- 💰 **Automated Fee Collection**: Platform fee collection and distribution (configurable basis points)
- 🔄 **Refund Support**: Built-in refund mechanism for disputed transactions
- 📊 **Event Tracking**: Rich event emission for payment analytics
- 🔐 **Secure Authorization**: Built-in authorization checks for all sensitive operations

## 📦 Contracts

### 1. Payment Contract (`payment-contract`)

Handles customer-to-merchant payment flows with support for multiple tokens.

**Key Functions:**
- `create_payment()` - Initialize a payment from customer to merchant
- `execute_payment()` - Execute token transfer from customer to merchant
- `refund_payment()` - Refund completed payments back to customers
- `get_payment()` - Query payment details by ID

**Payment Lifecycle:**
```
Customer → Create Payment → Execute Payment → Completed
                ↓
           Merchant can issue Refund
```

**Events Emitted:**
- `payment_created` - When payment is initialized
- `payment_completed` - When payment is executed
- `payment_refunded` - When payment is refunded

### 2. Merchant Registry Contract (`merchant-registry-contract`)

Manages merchant registration, profiles, and settlement configurations.

**Key Functions:**
- `initialize()` - Set contract admin
- `register_merchant()` - Register new merchant with business details
- `get_merchant()` - Get merchant profile
- `set_merchant_status()` - Activate/deactivate merchant
- `is_merchant_active()` - Check merchant active status

**Merchant Data:**
- Address & Settlement Address
- Business Name & ID
- Registration Timestamp
- Active Status

**Events Emitted:**
- `merchant_registered` - When merchant is registered
- `merchant_status_updated` - When merchant status changes

### 3. Settlement Contract (`settlement-contract`)

Handles settlement creation, fee calculation, and payout execution.

**Key Functions:**
- `initialize()` - Set platform fee (in basis points)
- `create_settlement()` - Create settlement with automatic fee calculation
- `execute_settlement()` - Execute payout to merchant and fee to platform
- `get_settlement()` - Query settlement details
- `get_platform_fee()` - Get current platform fee

**Fee Calculation:**
```rust
// Platform fee in basis points (50 = 0.5%)
fee = (amount × platform_fee) / 10000
net_amount = amount - fee
```

**Events Emitted:**
- `settlement_created` - When settlement is created
- `settlement_completed` - When settlement is executed

## 🚀 Installation

### Prerequisites

- **Rust**: 1.70 or higher
- **Soroban CLI**: Latest version
- **Stellar Account**: For testnet deployment

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Soroban CLI
cargo install --locked soroban-cli

# Add wasm32 target
rustup target add wasm32-unknown-unknown
```

### Clone and Build

```bash
# Clone repository
git clone https://github.com/SETTLIX/settlix-contracts.git
cd settlix-contracts

# Build all contracts
cargo build --target wasm32-unknown-unknown --release

# Run tests
cargo test
```

### Build Output

Compiled WASM files will be in:
```
target/wasm32-unknown-unknown/release/
├── payment_contract.wasm
├── merchant_registry_contract.wasm
└── settlement_contract.wasm
```

## 📖 Usage

### 1. Deploy Contracts

```bash
# Set network
NETWORK="testnet"
RPC_URL="https://soroban-testnet.stellar.org"
NETWORK_PASSPHRASE="Test SDF Network ; September 2015"

# Deploy Payment Contract
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/payment_contract.wasm \
  --source ADMIN_SECRET \
  --rpc-url $RPC_URL \
  --network-passphrase "$NETWORK_PASSPHRASE"

# Save contract ID
PAYMENT_CONTRACT_ID="CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC"
```

### 2. Initialize Contracts

```bash
# Initialize Merchant Registry
soroban contract invoke \
  --id $MERCHANT_REGISTRY_ID \
  --source ADMIN_SECRET \
  --rpc-url $RPC_URL \
  --network-passphrase "$NETWORK_PASSPHRASE" \
  -- initialize \
  --admin GADMIN_ADDRESS

# Initialize Settlement Contract with 0.5% fee
soroban contract invoke \
  --id $SETTLEMENT_CONTRACT_ID \
  --source ADMIN_SECRET \
  --rpc-url $RPC_URL \
  --network-passphrase "$NETWORK_PASSPHRASE" \
  -- initialize \
  --platform_fee 50
```

### 3. Register Merchant

```bash
soroban contract invoke \
  --id $MERCHANT_REGISTRY_ID \
  --source ADMIN_SECRET \
  -- register_merchant \
  --merchant GMERCHANT_ADDRESS \
  --name "Coffee Shop" \
  --business_id "BIZ-12345" \
  --settlement_address GSETTLEMENT_ADDRESS
```

### 4. Create and Execute Payment

```bash
# Customer creates payment
soroban contract invoke \
  --id $PAYMENT_CONTRACT_ID \
  --source CUSTOMER_SECRET \
  -- create_payment \
  --customer GCUSTOMER_ADDRESS \
  --merchant GMERCHANT_ADDRESS \
  --amount 10000000 \
  --token USDC_TOKEN_ADDRESS \
  --reference "ORDER-123"

# Execute payment (transfers tokens)
soroban contract invoke \
  --id $PAYMENT_CONTRACT_ID \
  --source CUSTOMER_SECRET \
  -- execute_payment \
  --payment_id 1
```

### 5. Create and Execute Settlement

```bash
# Merchant creates settlement
soroban contract invoke \
  --id $SETTLEMENT_CONTRACT_ID \
  --source MERCHANT_SECRET \
  -- create_settlement \
  --merchant GMERCHANT_ADDRESS \
  --amount 10000000 \
  --token USDC_TOKEN_ADDRESS

# Platform executes settlement (pays merchant + fee)
soroban contract invoke \
  --id $SETTLEMENT_CONTRACT_ID \
  --source PLATFORM_SECRET \
  -- execute_settlement \
  --settlement_id 1 \
  --platform_wallet GPLATFORM_ADDRESS
```

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run specific contract tests
cargo test -p payment-contract
cargo test -p merchant-registry-contract
cargo test -p settlement-contract

# Run with output
cargo test -- --nocapture
```

## 📂 Project Structure

```
settlix-contracts/
├── Cargo.toml                    # Workspace configuration
├── contracts/
│   ├── payment/                  # Payment processing
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs           # Payment contract logic
│   ├── merchant_registry/        # Merchant management
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs           # Registry contract logic
│   └── settlement/               # Settlement & payouts
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs           # Settlement contract logic
└── README.md
```

## 🔒 Security Considerations

- ✅ All sensitive operations require explicit authorization via `require_auth()`
- ✅ Overflow-checked arithmetic in release builds
- ✅ Status validation prevents double-processing of payments/settlements
- ✅ Token transfers use Soroban's built-in token interface
- ⚠️ Always test on testnet before mainnet deployment
- ⚠️ Audit contract code before production use

## 🛠️ Development

### Running Tests

```bash
cargo test
```

### Building Optimized WASM

```bash
cargo build --target wasm32-unknown-unknown --release --profile release
```

### Inspecting Contract

```bash
soroban contract inspect --wasm target/wasm32-unknown-unknown/release/payment_contract.wasm
```

## 📊 Gas Optimization

All contracts are configured with:
- `opt-level = "z"` - Maximum size optimization
- `lto = true` - Link-time optimization
- `codegen-units = 1` - Single codegen unit for better optimization
- `strip = "symbols"` - Remove debug symbols

## 🤝 Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License.

## 🔗 Links

- **Organization**: [SETTLIX](https://github.com/SETTLIX)
- **Interface**: [settlix-interface](https://github.com/SETTLIX/settlix-interface)
- **Oracle**: [settlix-oracle](https://github.com/SETTLIX/settlix-oracle)
- **Stellar Docs**: [soroban.stellar.org](https://soroban.stellar.org)

---

Built with ❤️ for the Stellar ecosystem
