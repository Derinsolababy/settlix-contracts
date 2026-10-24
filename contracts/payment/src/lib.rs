//! Payment: merchant invoices, paid atomically.
//!
//! 1. A merchant creates an invoice (token, amount, its own reference, an
//!    expiry). The QR code a customer scans points at the invoice id.
//! 2. The customer pays it in one transaction: the registry confirms the
//!    merchant is active and the token is allowed, the platform fee goes to
//!    the settlement vault, and the net goes to the merchant's settlement
//!    address. The contract never holds funds between steps.
//! 3. The merchant's settlement address can refund, in part or in full.
//!
//! A merchant reference is unique, so the same order cannot be invoiced (and
//! paid) twice, and an invoice can only be paid once.

#![no_std]

use shared::{extend_instance, extend_persistent, split_fee};
use soroban_sdk::{
    contract, contractclient, contracterror, contractevent, contractimpl, contracttype, token,
    Address, Env, String,
};

pub const MAX_REFERENCE_LEN: u32 = 64;

#[allow(dead_code)]
#[contractclient(name = "RegistryClient")]
pub trait Registry {
    fn payee(env: Env, merchant: Address) -> Option<Address>;
    fn is_token_allowed(env: Env, token: Address) -> bool;
}

#[allow(dead_code)]
#[contractclient(name = "SettlementClient")]
pub trait Settlement {
    fn fee_bps(env: Env) -> u32;
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PaymentError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Paused = 3,
    InvalidAmount = 4,
    InvalidReference = 5,
    DuplicateReference = 6,
    MerchantNotActive = 7,
    TokenNotAllowed = 8,
    InvoiceNotFound = 9,
    NotOpen = 10,
    Expired = 11,
    InvalidExpiry = 12,
    RefundTooLarge = 13,
    Overflow = 14,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum InvoiceStatus {
    Open,
    Paid,
    Cancelled,
    PartiallyRefunded,
    Refunded,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Invoice {
    pub id: u64,
    pub merchant: Address,
    pub token: Address,
    pub amount: i128,
    pub reference: String,
    pub expires_at: u64,
    pub status: InvoiceStatus,
    pub created_at: u64,
    /// Set when paid.
    pub customer: Option<Address>,
    pub payee: Option<Address>,
    pub fee: i128,
    pub refunded: i128,
    pub paid_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub admin: Address,
    pub registry: Address,
    pub settlement: Address,
    pub paused: bool,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Config,
    Count,
    Invoice(u64),
    Reference(Address, String),
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvoiceCreated {
    #[topic]
    pub merchant: Address,
    #[topic]
    pub id: u64,
    pub token: Address,
    pub amount: i128,
    pub reference: String,
    pub expires_at: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvoicePaid {
    #[topic]
    pub merchant: Address,
    #[topic]
    pub id: u64,
    pub customer: Address,
    pub amount: i128,
    pub fee: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvoiceCancelled {
    #[topic]
    pub id: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvoiceRefunded {
    #[topic]
    pub id: u64,
    pub amount: i128,
    pub total_refunded: i128,
}

soroban_sdk::contractmeta!(
    key = "source_repo",
    val = "github:SETTLIX/settlix-contracts"
);
soroban_sdk::contractmeta!(key = "desc", val = "Settlix invoices and payments");

#[contract]
pub struct PaymentContract;

#[contractimpl]
impl PaymentContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        registry: Address,
        settlement: Address,
    ) -> Result<(), PaymentError> {
        if env.storage().instance().has(&DataKey::Config) {
            return Err(PaymentError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(
            &DataKey::Config,
            &Config {
                admin,
                registry,
                settlement,
                paused: false,
            },
        );
        extend_instance(&env);
        Ok(())
    }

    /// Merchant opens an invoice payable until `expires_at`.
    pub fn create_invoice(
        env: Env,
        merchant: Address,
        token: Address,
        amount: i128,
        reference: String,
        expires_at: u64,
    ) -> Result<u64, PaymentError> {
        let config = Self::live_config(&env)?;
        merchant.require_auth();
        if amount <= 0 {
            return Err(PaymentError::InvalidAmount);
        }
        if reference.is_empty() || reference.len() > MAX_REFERENCE_LEN {
            return Err(PaymentError::InvalidReference);
        }
        let now = env.ledger().timestamp();
        if expires_at <= now {
            return Err(PaymentError::InvalidExpiry);
        }
        let registry = RegistryClient::new(&env, &config.registry);
        if registry.payee(&merchant).is_none() {
            return Err(PaymentError::MerchantNotActive);
        }
        if !registry.is_token_allowed(&token) {
            return Err(PaymentError::TokenNotAllowed);
        }
        let ref_key = DataKey::Reference(merchant.clone(), reference.clone());
        if env.storage().persistent().has(&ref_key) {
            return Err(PaymentError::DuplicateReference);
        }

        let id: u64 = env.storage().instance().get(&DataKey::Count).unwrap_or(0) + 1;
        Self::save(
            &env,
            &Invoice {
                id,
                merchant: merchant.clone(),
                token: token.clone(),
                amount,
                reference: reference.clone(),
                expires_at,
                status: InvoiceStatus::Open,
                created_at: now,
                customer: None,
                payee: None,
                fee: 0,
                refunded: 0,
                paid_at: 0,
            },
        );
        env.storage().persistent().set(&ref_key, &id);
        extend_persistent(&env, &ref_key);
        env.storage().instance().set(&DataKey::Count, &id);
        extend_instance(&env);

        InvoiceCreated {
            merchant,
            id,
            token,
            amount,
            reference,
            expires_at,
        }
        .publish(&env);
        Ok(id)
    }

    /// Customer pays an open invoice: fee to the settlement vault, net to the
    /// merchant's current settlement address, all in this transaction.
    pub fn pay_invoice(env: Env, customer: Address, id: u64) -> Result<(), PaymentError> {
        let config = Self::live_config(&env)?;
        customer.require_auth();
        let mut invoice = Self::load(&env, id)?;
        if invoice.status != InvoiceStatus::Open {
            return Err(PaymentError::NotOpen);
        }
        if env.ledger().timestamp() >= invoice.expires_at {
            return Err(PaymentError::Expired);
        }
        // Re-check at payment time: the merchant may have been suspended.
        let payee = RegistryClient::new(&env, &config.registry)
            .payee(&invoice.merchant)
            .ok_or(PaymentError::MerchantNotActive)?;
        let fee_bps = SettlementClient::new(&env, &config.settlement).fee_bps();
        let (fee, net) = split_fee(invoice.amount, fee_bps).ok_or(PaymentError::Overflow)?;

        let token = token::Client::new(&env, &invoice.token);
        token.transfer(&customer, &payee, &net);
        if fee > 0 {
            token.transfer(&customer, &config.settlement, &fee);
        }

        invoice.status = InvoiceStatus::Paid;
        invoice.customer = Some(customer.clone());
        invoice.payee = Some(payee);
        invoice.fee = fee;
        invoice.paid_at = env.ledger().timestamp();
        Self::save(&env, &invoice);

        InvoicePaid {
            merchant: invoice.merchant,
            id,
            customer,
            amount: invoice.amount,
            fee,
        }
        .publish(&env);
        Ok(())
    }

    pub fn cancel_invoice(env: Env, id: u64) -> Result<(), PaymentError> {
        let mut invoice = Self::load(&env, id)?;
        invoice.merchant.require_auth();
        if invoice.status != InvoiceStatus::Open {
            return Err(PaymentError::NotOpen);
        }
        invoice.status = InvoiceStatus::Cancelled;
        Self::save(&env, &invoice);
        InvoiceCancelled { id }.publish(&env);
        Ok(())
    }

    /// The account that received the money refunds the customer, up to the
    /// invoice amount in total. The platform fee is not returned.
    pub fn refund(env: Env, id: u64, amount: i128) -> Result<i128, PaymentError> {
        let mut invoice = Self::load(&env, id)?;
        let (customer, payee) = match (&invoice.customer, &invoice.payee) {
            (Some(c), Some(p))
                if matches!(
                    invoice.status,
                    InvoiceStatus::Paid | InvoiceStatus::PartiallyRefunded
                ) =>
            {
                (c.clone(), p.clone())
            }
            _ => return Err(PaymentError::NotOpen),
        };
        payee.require_auth();
        if amount <= 0 {
            return Err(PaymentError::InvalidAmount);
        }
        let total = invoice
            .refunded
            .checked_add(amount)
            .ok_or(PaymentError::Overflow)?;
        if total > invoice.amount {
            return Err(PaymentError::RefundTooLarge);
        }
        token::Client::new(&env, &invoice.token).transfer(&payee, &customer, &amount);
        invoice.refunded = total;
        invoice.status = if total == invoice.amount {
            InvoiceStatus::Refunded
        } else {
            InvoiceStatus::PartiallyRefunded
        };
        Self::save(&env, &invoice);
        InvoiceRefunded {
            id,
            amount,
            total_refunded: total,
        }
        .publish(&env);
        Ok(total)
    }

    pub fn pause(env: Env) -> Result<(), PaymentError> {
        Self::set_paused(&env, true)
    }

    pub fn unpause(env: Env) -> Result<(), PaymentError> {
        Self::set_paused(&env, false)
    }

    // ---------------------------------------------------------------- views

    pub fn get_invoice(env: Env, id: u64) -> Result<Invoice, PaymentError> {
        Self::load(&env, id)
    }

    pub fn invoice_by_reference(
        env: Env,
        merchant: Address,
        reference: String,
    ) -> Result<Invoice, PaymentError> {
        let id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Reference(merchant, reference))
            .ok_or(PaymentError::InvoiceNotFound)?;
        Self::load(&env, id)
    }

    pub fn invoice_count(env: Env) -> u64 {
        env.storage().instance().get(&DataKey::Count).unwrap_or(0)
    }

    pub fn get_config(env: Env) -> Result<Config, PaymentError> {
        Self::config(&env)
    }

    // ------------------------------------------------------------ internals

    fn config(env: &Env) -> Result<Config, PaymentError> {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(PaymentError::NotInitialized)
    }

    fn live_config(env: &Env) -> Result<Config, PaymentError> {
        let config = Self::config(env)?;
        if config.paused {
            return Err(PaymentError::Paused);
        }
        Ok(config)
    }

    fn set_paused(env: &Env, paused: bool) -> Result<(), PaymentError> {
        let mut config = Self::config(env)?;
        config.admin.require_auth();
        config.paused = paused;
        env.storage().instance().set(&DataKey::Config, &config);
        extend_instance(env);
        Ok(())
    }

    fn load(env: &Env, id: u64) -> Result<Invoice, PaymentError> {
        let key = DataKey::Invoice(id);
        let invoice = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(PaymentError::InvoiceNotFound)?;
        extend_persistent(env, &key);
        Ok(invoice)
    }

    fn save(env: &Env, invoice: &Invoice) {
        let key = DataKey::Invoice(invoice.id);
        env.storage().persistent().set(&key, invoice);
        extend_persistent(env, &key);
    }
}

#[cfg(test)]
mod test;
