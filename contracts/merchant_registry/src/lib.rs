//! Merchant registry.
//!
//! Merchants apply with their own signature; the platform admin approves
//! them after off-chain verification (KYC). Only a SHA-256 hash of the
//! business identifier is stored, so no personal or company data is put on
//! a public ledger, yet anyone holding the documents can prove the match.
//! The admin also keeps the list of tokens the platform settles in.
//!
//! Status: `Pending` -(approve)-> `Active` <-(suspend/reinstate)-> `Suspended`.
//! `reject` deletes a pending application.

#![no_std]

use shared::{extend_instance, extend_persistent};
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, Address, BytesN, Env,
    String,
};

pub const MAX_NAME_LEN: u32 = 64;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RegistryError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    AlreadyApplied = 3,
    MerchantNotFound = 4,
    InvalidName = 5,
    InvalidTransition = 6,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MerchantStatus {
    Pending,
    Active,
    Suspended,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Merchant {
    pub address: Address,
    pub name: String,
    /// SHA-256 of the business registration ID (never the ID itself).
    pub business_hash: BytesN<32>,
    /// Account that receives invoice payments.
    pub settlement_address: Address,
    pub status: MerchantStatus,
    pub registered_at: u64,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    Merchant(Address),
    Token(Address),
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MerchantApplied {
    #[topic]
    pub merchant: Address,
    pub name: String,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MerchantStatusChanged {
    #[topic]
    pub merchant: Address,
    pub status: MerchantStatus,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenAllowed {
    #[topic]
    pub token: Address,
    pub allowed: bool,
}

soroban_sdk::contractmeta!(
    key = "source_repo",
    val = "github:SETTLIX/settlix-contracts"
);
soroban_sdk::contractmeta!(key = "desc", val = "Settlix merchant registry");

#[contract]
pub struct MerchantRegistryContract;

#[contractimpl]
impl MerchantRegistryContract {
    pub fn initialize(env: Env, admin: Address) -> Result<(), RegistryError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(RegistryError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        extend_instance(&env);
        Ok(())
    }

    /// A merchant applies; it is `Pending` until approved.
    pub fn apply(
        env: Env,
        merchant: Address,
        name: String,
        business_hash: BytesN<32>,
        settlement_address: Address,
    ) -> Result<(), RegistryError> {
        merchant.require_auth();
        if name.is_empty() || name.len() > MAX_NAME_LEN {
            return Err(RegistryError::InvalidName);
        }
        let key = DataKey::Merchant(merchant.clone());
        if env.storage().persistent().has(&key) {
            return Err(RegistryError::AlreadyApplied);
        }
        Self::save(
            &env,
            &Merchant {
                address: merchant.clone(),
                name: name.clone(),
                business_hash,
                settlement_address,
                status: MerchantStatus::Pending,
                registered_at: env.ledger().timestamp(),
            },
        );
        MerchantApplied { merchant, name }.publish(&env);
        Ok(())
    }

    /// Merchants move their own payouts. The new account must also sign, so
    /// payouts cannot be pointed at an address nobody controls.
    pub fn set_settlement_address(
        env: Env,
        merchant: Address,
        address: Address,
    ) -> Result<(), RegistryError> {
        merchant.require_auth();
        address.require_auth();
        let mut m = Self::load(&env, &merchant)?;
        m.settlement_address = address;
        Self::save(&env, &m);
        Ok(())
    }

    pub fn approve(env: Env, merchant: Address) -> Result<(), RegistryError> {
        Self::transition(
            &env,
            merchant,
            MerchantStatus::Pending,
            MerchantStatus::Active,
        )
    }

    pub fn suspend(env: Env, merchant: Address) -> Result<(), RegistryError> {
        Self::transition(
            &env,
            merchant,
            MerchantStatus::Active,
            MerchantStatus::Suspended,
        )
    }

    pub fn reinstate(env: Env, merchant: Address) -> Result<(), RegistryError> {
        Self::transition(
            &env,
            merchant,
            MerchantStatus::Suspended,
            MerchantStatus::Active,
        )
    }

    /// Declines a pending application so the merchant can fix it and reapply.
    pub fn reject(env: Env, merchant: Address) -> Result<(), RegistryError> {
        Self::admin(&env)?.require_auth();
        if Self::load(&env, &merchant)?.status != MerchantStatus::Pending {
            return Err(RegistryError::InvalidTransition);
        }
        env.storage()
            .persistent()
            .remove(&DataKey::Merchant(merchant));
        Ok(())
    }

    pub fn set_token_allowed(env: Env, token: Address, allowed: bool) -> Result<(), RegistryError> {
        Self::admin(&env)?.require_auth();
        let key = DataKey::Token(token.clone());
        if allowed {
            env.storage().persistent().set(&key, &true);
            extend_persistent(&env, &key);
        } else {
            env.storage().persistent().remove(&key);
        }
        TokenAllowed { token, allowed }.publish(&env);
        Ok(())
    }

    pub fn transfer_admin(env: Env, new_admin: Address) -> Result<(), RegistryError> {
        Self::admin(&env)?.require_auth();
        new_admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        extend_instance(&env);
        Ok(())
    }

    // ---------------------------------------------------------------- views

    pub fn get_merchant(env: Env, merchant: Address) -> Result<Merchant, RegistryError> {
        Self::load(&env, &merchant)
    }

    /// Where an active merchant gets paid; `None` if not payable.
    pub fn payee(env: Env, merchant: Address) -> Option<Address> {
        let m = Self::load(&env, &merchant).ok()?;
        (m.status == MerchantStatus::Active).then_some(m.settlement_address)
    }

    pub fn is_token_allowed(env: Env, token: Address) -> bool {
        env.storage().persistent().has(&DataKey::Token(token))
    }

    // ------------------------------------------------------------ internals

    fn admin(env: &Env) -> Result<Address, RegistryError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(RegistryError::NotInitialized)
    }

    fn transition(
        env: &Env,
        merchant: Address,
        from: MerchantStatus,
        to: MerchantStatus,
    ) -> Result<(), RegistryError> {
        Self::admin(env)?.require_auth();
        let mut m = Self::load(env, &merchant)?;
        if m.status != from {
            return Err(RegistryError::InvalidTransition);
        }
        m.status = to;
        Self::save(env, &m);
        MerchantStatusChanged {
            merchant,
            status: to,
        }
        .publish(env);
        Ok(())
    }

    fn load(env: &Env, merchant: &Address) -> Result<Merchant, RegistryError> {
        let key = DataKey::Merchant(merchant.clone());
        let m = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(RegistryError::MerchantNotFound)?;
        extend_persistent(env, &key);
        Ok(m)
    }

    fn save(env: &Env, m: &Merchant) {
        let key = DataKey::Merchant(m.address.clone());
        env.storage().persistent().set(&key, m);
        extend_persistent(env, &key);
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::*;
    use soroban_sdk::{testutils::Address as _, BytesN, Env};

    fn setup() -> (
        Env,
        MerchantRegistryContractClient<'static>,
        Address,
        Address,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let c =
            MerchantRegistryContractClient::new(&env, &env.register(MerchantRegistryContract, ()));
        c.initialize(&Address::generate(&env));
        let merchant = Address::generate(&env);
        let payout = Address::generate(&env);
        (env, c, merchant, payout)
    }

    fn apply(env: &Env, c: &MerchantRegistryContractClient, merchant: &Address, payout: &Address) {
        c.apply(
            merchant,
            &String::from_str(env, "Mama Put Kitchen"),
            &BytesN::from_array(env, &[7; 32]),
            payout,
        );
    }

    #[test]
    fn only_approved_merchants_are_payable() {
        let (env, c, m, payout) = setup();
        apply(&env, &c, &m, &payout);
        assert_eq!(c.payee(&m), None);
        c.approve(&m);
        assert_eq!(c.payee(&m), Some(payout.clone()));
        c.suspend(&m);
        assert_eq!(c.payee(&m), None);
        c.reinstate(&m);
        assert_eq!(c.payee(&m), Some(payout));
    }

    #[test]
    fn stores_only_the_business_hash() {
        let (env, c, m, payout) = setup();
        apply(&env, &c, &m, &payout);
        assert_eq!(
            c.get_merchant(&m).business_hash,
            BytesN::from_array(&env, &[7; 32])
        );
    }

    #[test]
    fn applications_are_validated_and_unique() {
        let (env, c, m, payout) = setup();
        assert_eq!(
            c.try_apply(
                &m,
                &String::from_str(&env, ""),
                &BytesN::from_array(&env, &[0; 32]),
                &payout
            ),
            Err(Ok(RegistryError::InvalidName))
        );
        apply(&env, &c, &m, &payout);
        assert_eq!(
            c.try_apply(
                &m,
                &String::from_str(&env, "Again"),
                &BytesN::from_array(&env, &[0; 32]),
                &payout
            ),
            Err(Ok(RegistryError::AlreadyApplied))
        );
    }

    #[test]
    fn transitions_are_enforced_and_rejection_allows_reapplying() {
        let (env, c, m, payout) = setup();
        apply(&env, &c, &m, &payout);
        assert_eq!(c.try_suspend(&m), Err(Ok(RegistryError::InvalidTransition)));
        c.reject(&m);
        assert_eq!(
            c.try_get_merchant(&m),
            Err(Ok(RegistryError::MerchantNotFound))
        );
        apply(&env, &c, &m, &payout);
        c.approve(&m);
        assert_eq!(c.try_reject(&m), Err(Ok(RegistryError::InvalidTransition)));
    }

    #[test]
    fn merchants_move_their_own_payouts() {
        let (env, c, m, payout) = setup();
        apply(&env, &c, &m, &payout);
        c.approve(&m);
        let new_payout = Address::generate(&env);
        c.set_settlement_address(&m, &new_payout);
        assert_eq!(c.payee(&m), Some(new_payout));
    }

    #[test]
    fn token_allowlist() {
        let (env, c, _m, _payout) = setup();
        let usdc = Address::generate(&env);
        assert!(!c.is_token_allowed(&usdc));
        c.set_token_allowed(&usdc, &true);
        assert!(c.is_token_allowed(&usdc));
        c.set_token_allowed(&usdc, &false);
        assert!(!c.is_token_allowed(&usdc));
    }

    #[test]
    fn initialize_once() {
        let (env, c, _m, _payout) = setup();
        assert_eq!(
            c.try_initialize(&Address::generate(&env)),
            Err(Ok(RegistryError::AlreadyInitialized))
        );
    }
}
